use crate::terrain::{CH, CW, Cell, HH, NH, NW, Terrain, WW, hash};
use bytemuck::{Pod, Zeroable};
use std::{borrow::Cow, sync::Arc};
use wgpu::util::DeviceExt;
#[path = "hud.rs"]
pub mod hud;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Params {
    pub view: [f32; 4],
    pub screen: [f32; 4],
    pub sim: [u32; 4],
    pub world: [u32; 4],
    pub misc: [f32; 4],
    pub counts: [u32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Agent {
    pub pos: [f32; 2],
    pub facing: f32,
    pub id: u32,
    pub goal: u32,
    // Faction for troops; tagged RGB/atlas-family appearance for rendered fauna.
    pub team: u32,
    pub role: u32,
    pub next: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, serde::Serialize, serde::Deserialize)]
pub struct Object {
    pub pos: [f32; 2],
    pub size: f32,
    pub kind: u32,
    pub uv: [u32; 4],
    pub team: u32,
    pub height: f32,
    pub complete: f32,
    pub seed: u32,
}
pub const CAPACITY: usize = 100000;
pub const OBJECT_CAPACITY: usize = 4096;
pub const CAMPAIGN_MASK_CELLS: usize = 15000;
const UI_QUAD_CAPACITY: usize = 131072;
pub struct UiEvidenceFrame {
    pub width: u32,
    pub height: u32,
    pub label: String,
    pub model: hud::HudModel,
}
#[derive(Default, serde::Serialize)]
struct HudDiagnostics {
    model_cache_hits: u64,
    updates: u64,
    layers_redrawn: u64,
    layers_reused: u64,
    last_cpu_ms: f64,
    last_upload_bytes: u64,
    total_upload_bytes: u64,
    last_dirty_rects: usize,
    world_quads: usize,
}

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    pub params: Params,
    pub buffers: Vec<wgpu::Buffer>,
    compute_group: wgpu::BindGroup,
    render_group: wgpu::BindGroup,
    pipelines: Vec<wgpu::ComputePipeline>,
    render: wgpu::RenderPipeline,
    troops: wgpu::RenderPipeline,
    campaign_objects: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    hud_texture: wgpu::Texture,
    hud_model: Option<(hud::HudModel, u32, u32)>,
    hud_regions: Vec<hud::Rect>,
    hud_raster: hud::RasterCache,
    hud_diagnostics: HudDiagnostics,
    hud_cpu_timings: crate::metrics::FrameTimings,
    ui_world_pipeline: wgpu::RenderPipeline,
    ui_world_buffer: wgpu::Buffer,
    ui_world_count: u32,
    campaign_mode: bool,
    ground_texture: wgpu::Texture,
    pub format: wgpu::TextureFormat,
    pub generated_ms: f64,
    last_count: u32,
    last_time: f32,
    terrain_revision: u64,
    vision_dirty: bool,
    mask_geometry_dirty: bool,
}
fn decode_png(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("Embedded atlas header");
    let mut out = vec![0; reader.output_buffer_size().expect("Atlas dimensions")];
    let info = reader.next_frame(&mut out).expect("Atlas pixels");
    out.truncate(info.buffer_size());
    (out, info.width, info.height)
}

/// Select an existing procedural grown material. This does not create artwork.
/// Callers choose explicit reuse (e.g. wall material for a dam) rather than a
/// missing atlas name silently turning into a different structure.
pub fn grown_object(
    kind: &str,
    team: u32,
    pos: [f32; 2],
    size: f32,
    height: f32,
    complete: f32,
    seed: u32,
) -> Result<Object, String> {
    static MANIFEST: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let manifest = MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/pixel-atlas.json"))
            .expect("embedded procedural atlas metadata")
    });
    let rect = manifest["rects"]
        .get(format!("{kind}:{team}:{}", seed % 4))
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("No grown material for {kind:?}, team {team}"))?;
    let mut uv = [0; 4];
    for (i, value) in rect.iter().enumerate().take(4) {
        uv[i] = value
            .as_u64()
            .ok_or("Invalid embedded material rectangle")? as u32;
    }
    let object = Object {
        pos,
        size,
        kind: 0,
        uv,
        team,
        height,
        complete,
        seed,
    };
    validate_campaign_objects(&[object], 1)?;
    Ok(object)
}

/// Resolve every campaign construction material before starting its renderer.
/// Jobs can expose any tool's material before the corresponding building exists.
fn prewarm_campaign_materials() -> Result<(), String> {
    for kind in crate::construction::BuildKind::ALL {
        for team in 0..2 {
            for variant in 0..4 {
                grown_object(kind.atlas_kind(), team, [100., 100.], 32., 4., 0.5, variant)
                    .map_err(|e| format!("Campaign material preflight for {}: {e}", kind.name()))?;
            }
        }
    }
    Ok(())
}

fn validate_campaign_agents(agents: &[Agent]) -> Result<(), String> {
    if agents.len() > CAPACITY {
        return Err(format!(
            "{} render agents exceed capacity {CAPACITY}; no count was reduced",
            agents.len()
        ));
    }
    for a in agents {
        if !a.pos.iter().all(|v| v.is_finite())
            || !a.facing.is_finite()
            || a.pos[0] < 0.
            || a.pos[1] < 0.
            || a.pos[0] >= WW as f32
            || a.pos[1] >= HH as f32
        {
            return Err(format!(
                "Render agent {} has an invalid position/facing",
                a.id
            ));
        }
        let valid_material = match a.role {
            0..=3 => a.team <= 1,
            4 => a.team <= 3 || crate::natural_colors::valid_fauna_appearance(a.team),
            5 => true,
            _ => false,
        };
        if !valid_material {
            return Err(format!(
                "Render agent {} has no material for role/team {}/{}",
                a.id, a.role, a.team
            ));
        }
    }
    Ok(())
}

fn validate_campaign_objects(objects: &[Object], physical_count: usize) -> Result<(), String> {
    if objects.len() > OBJECT_CAPACITY || physical_count > objects.len() {
        return Err("Campaign object count exceeds resident capacity or physical range".into());
    }
    for o in objects {
        if !o.pos.iter().all(|v| v.is_finite())
            || !o.size.is_finite()
            || o.size <= 0.
            || !o.height.is_finite()
            || o.height < 0.
            || !o.complete.is_finite()
            || !(0.0..=1.0).contains(&o.complete)
            || o.uv[2] == 0
            || o.uv[3] == 0
            || o.uv[0].checked_add(o.uv[2]).is_none_or(|end| end > 1024)
            || o.uv[1].checked_add(o.uv[3]).is_none_or(|end| end > 1024)
        {
            return Err(format!("Invalid campaign render object {}", o.seed));
        }
    }
    Ok(())
}
fn scene(t: &mut Terrain) -> (Vec<Object>, u32) {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../assets/pixel-atlas.json")).unwrap();
    let mut objects = Vec::new();
    let mut sensors = Vec::new();
    let definitions = [
        ("ship", 424., 1640., 0, 108., 8.),
        ("ship", 4376., 1712., 1, 108., 8.),
        ("depot", 1616., 1608., 0, 49., 5.),
        ("mine", 1560., 1424., 0, 43., 5.),
        ("mill", 1496., 1792., 0, 43., 5.),
        ("barracks", 1712., 1784., 0, 59., 5.),
        ("solar", 1552., 1904., 0, 54., 2.),
        ("solar", 1664., 1904., 0, 54., 2.),
        ("farm", 1608., 1808., 0, 54., 3.),
        ("camera", 1480., 1656., 0, 22., 8.),
        ("radar", 1944., 1696., 0, 43., 6.),
        ("tower", 2144., 1696., 1, 43., 16.),
        ("depot", 2640., 1720., 1, 49., 5.),
        ("barracks", 2704., 1856., 1, 59., 5.),
        ("beacon", 2400., 1600., 0, 44., 3.),
    ];
    for (i, (kind, x, y, team, size, height)) in definitions.iter().enumerate() {
        let value = &manifest["rects"][format!("{kind}:{team}:{}", i % 4)];
        let mut uv = [0; 4];
        for j in 0..4 {
            uv[j] = value[j].as_u64().unwrap() as u32;
        }
        let object = Object {
            pos: [*x, *y],
            size: *size,
            kind: i as u32,
            uv,
            team: *team,
            height: *height,
            complete: 1.,
            seed: i as u32,
        };
        objects.push(object);
        // Coarse broad phase, with exact grown alpha checked by the GPU for sight.
        let radius = *size / 2.;
        for cy in ((*y - radius).max(0.) / 8.) as usize..=((*y + radius) / 8.) as usize {
            for cx in ((*x - radius).max(0.) / 8.) as usize..=((*x + radius) / 8.) as usize {
                if cx < CW && cy < CH {
                    t.cells[cy * CW + cx].object = i as u32 + 1;
                }
            }
        }
        if *team == 0 {
            let range = match *kind {
                "ship" => 520.,
                "camera" => 420.,
                "radar" => 950.,
                _ => 0.,
            };
            if range > 0. {
                sensors.push(Object {
                    size: range,
                    kind: if *kind == "radar" { 101 } else { 100 },
                    ..object
                });
            }
        }
    }
    t.road([424., 1640.], [4000., 1640.]);
    t.road([4000., 1640.], [4376., 1712.]);
    t.road([1560., 1424.], [1560., 1840.]);
    t.road([1496., 1840.], [1720., 1840.]);
    let count = objects.len() as u32;
    objects.extend(sensors);
    (objects, count)
}
impl Gpu {
    pub async fn new(
        window: Option<Arc<winit::window::Window>>,
        seed: u32,
        count: u32,
        width: u32,
        height: u32,
        backends: Option<wgpu::Backends>,
    ) -> Result<(Self, Option<wgpu::Surface<'static>>, Terrain), String> {
        Self::new_mode(window, seed, count, width, height, backends, false).await
    }

    /// The campaign uses the same device, textures, atlas and render pipelines.
    /// It starts without benchmark structures/roads or GPU-owned movement.
    pub async fn new_campaign(
        window: Option<Arc<winit::window::Window>>,
        seed: u32,
        width: u32,
        height: u32,
        backends: Option<wgpu::Backends>,
    ) -> Result<(Self, Option<wgpu::Surface<'static>>, Terrain), String> {
        prewarm_campaign_materials()?;
        Self::new_mode(window, seed, 0, width, height, backends, true).await
    }

    async fn new_mode(
        window: Option<Arc<winit::window::Window>>,
        seed: u32,
        count: u32,
        width: u32,
        height: u32,
        backends: Option<wgpu::Backends>,
        campaign_mode: bool,
    ) -> Result<(Self, Option<wgpu::Surface<'static>>, Terrain), String> {
        let started = web_time::Instant::now();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: backends.unwrap_or(
                wgpu::Backends::VULKAN
                    | wgpu::Backends::DX12
                    | wgpu::Backends::METAL
                    | wgpu::Backends::BROWSER_WEBGPU,
            ),
            ..Default::default()
        });
        let surface = window
            .map(|w| instance.create_surface(w))
            .transpose()
            .map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("No compute-capable graphics adapter: {e}"))?;
        let info = adapter.get_info();
        crate::logging::log("ADAPTER", &format!("{:?}", info));
        let supported = adapter.limits();
        if supported.max_storage_buffers_per_shader_stage < 6 {
            return Err(
                "This graphics adapter does not support the required six storage buffer bindings."
                    .into(),
            );
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Last World GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(supported),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        device.on_uncaptured_error(Arc::new(|error| {
            crate::logging::log("GPU_ERROR", &error.to_string())
        }));
        let mut terrain = Terrain::new(seed);
        let (objects, object_count) = if campaign_mode {
            (Vec::new(), 0)
        } else {
            scene(&mut terrain)
        };
        let mut flows = Vec::new();
        for goal in [
            [700., 1640.],
            [1800., 1640.],
            [3000., 1712.],
            [4000., 1712.],
        ] {
            if campaign_mode {
                flows.resize(flows.len() + NW * NH, [0., 0.]);
            } else {
                flows.extend(terrain.flow(goal));
            }
        }
        let params = Params {
            view: [2000., 1640., 0., 0.],
            screen: [width as f32, height as f32, 0.9, 0.],
            sim: [count, 300, 200, 0],
            world: [600, 400, 150, 100],
            misc: [0.05, if campaign_mode { 1. } else { 0. }, 1., -1.],
            counts: [
                object_count,
                objects.len() as u32 - object_count,
                u32::from(campaign_mode),
                0,
            ],
        };
        let mut buffers = vec![
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Frame controls"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }),
        ];
        let mut storage = |label: &str, data: &[u8], extra| {
            buffers.push(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: data,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
                }),
            );
        };
        storage(
            "Procedural terrain cells",
            bytemuck::cast_slice::<Cell, u8>(&terrain.cells),
            wgpu::BufferUsages::empty(),
        );
        storage(
            "Shared route directions",
            bytemuck::cast_slice(&flows),
            wgpu::BufferUsages::empty(),
        );
        let mut agents = Vec::with_capacity(CAPACITY);
        for i in 0..CAPACITY {
            if campaign_mode {
                agents.push(Agent::zeroed());
                continue;
            }
            let team = (i % 2) as u32;
            let x = if team == 0 { 1750. } else { 2450. };
            let y = if team == 0 { 1640. } else { 1712. };
            let mut pos = [
                x + (hash(i as i32, 3, seed) as f32 - 0.5) * 440.,
                y + (hash(i as i32, 7, seed) as f32 - 0.5) * 160.,
            ];
            let cell = terrain.cells
                [(pos[1] as usize / 8).min(CH - 1) * CW + (pos[0] as usize / 8).min(CW - 1)];
            if cell.water > 0 || cell.object > 0 {
                pos[1] = 1640.;
            }
            agents.push(Agent {
                pos,
                facing: 0.,
                id: i as u32,
                goal: if team == 0 { 2 } else { 1 },
                team,
                role: if i % 7 == 0 { 0 } else { 1 },
                next: -1,
            });
        }
        storage(
            "GPU-owned agents",
            bytemuck::cast_slice(&agents),
            wgpu::BufferUsages::COPY_SRC,
        );
        storage(
            "Spatial heads",
            bytemuck::cast_slice(&vec![-1i32; NW * NH]),
            wgpu::BufferUsages::COPY_SRC,
        );
        let mut resident_objects = objects.clone();
        resident_objects.resize(OBJECT_CAPACITY, Object::zeroed());
        storage(
            "Grown structures and camera emitters",
            bytemuck::cast_slice(&resident_objects),
            wgpu::BufferUsages::empty(),
        );
        storage(
            "Cached sight masks and exploration",
            bytemuck::cast_slice(&vec![0u32; 255000]),
            wgpu::BufferUsages::COPY_SRC,
        );
        let upload = |label: &str, rgba: &[u8], w: u32, h: u32| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            texture
        };
        let ground_texture = upload(
            "Seeded growth terrain",
            &terrain.rgba,
            (WW / 2) as u32,
            (HH / 2) as u32,
        );
        let ground = ground_texture.create_view(&Default::default());
        let (atlas_pixels, aw, ah) = decode_png(include_bytes!("../assets/pixel-atlas.png"));
        let atlas_texture = upload("Procedural cell materials", &atlas_pixels, aw, ah);
        let atlas = atlas_texture.create_view(&Default::default());
        let hud_texture = upload(
            "Screen text and controls",
            &vec![0; (hud::MAX_WIDTH * hud::MAX_HEIGHT * 4) as usize],
            hud::MAX_WIDTH,
            hud::MAX_HEIGHT,
        );
        let hud_view = hud_texture.create_view(&Default::default());
        let layout = |compute: bool| {
            let mut entries = Vec::new();
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: if compute {
                    wgpu::ShaderStages::COMPUTE
                } else {
                    wgpu::ShaderStages::VERTEX_FRAGMENT
                },
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            for i in 1..7 {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: if compute {
                        wgpu::ShaderStages::COMPUTE
                    } else {
                        wgpu::ShaderStages::VERTEX_FRAGMENT
                    },
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage {
                            read_only: !compute || ![3, 4, 6].contains(&i),
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
            for i in 7..9 {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: if compute {
                        wgpu::ShaderStages::COMPUTE
                    } else {
                        wgpu::ShaderStages::FRAGMENT
                    },
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                });
            }
            if !compute {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                });
            }
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Last World resident data layout"),
                entries: &entries,
            })
        };
        let compute_layout = layout(true);
        let render_layout = layout(false);
        let group = |layout: &wgpu::BindGroupLayout, with_hud: bool| {
            let mut entries: Vec<_> = buffers
                .iter()
                .enumerate()
                .map(|(i, b)| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: b.as_entire_binding(),
                })
                .collect();
            entries.push(wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&ground),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(&atlas),
            });
            if with_hud {
                entries.push(wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&hud_view),
                });
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Last World resident resources"),
                layout,
                entries: &entries,
            })
        };
        let compute_group = group(&compute_layout, false);
        let render_group = group(&render_layout, true);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let compute = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GPU movement, binning, sight"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../shaders/compute.wgsl"
            ))),
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&compute_layout],
                push_constant_ranges: &[],
            });
        let mut pipelines = Vec::new();
        for name in [
            "clear",
            "movement",
            "binning",
            "sensor_masks",
            "combine_vision",
        ] {
            pipelines.push(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(name),
                    layout: Some(&compute_pipeline_layout),
                    module: &compute,
                    entry_point: Some(name),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            );
        }
        let render_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Pixel world renderer"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/render.wgsl"))),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let format = surface
            .as_ref()
            .map(|s| {
                s.get_capabilities(&adapter)
                    .formats
                    .into_iter()
                    .find(|f| !f.is_srgb())
                    .unwrap_or(wgpu::TextureFormat::Bgra8Unorm)
            })
            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm);
        let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GPU pixel scene"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let troops = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GPU instanced troop materials"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_module,
                entry_point: Some("troop_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_module,
                entry_point: Some("troop_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let overlay_pipeline = |label, vertex, fragment, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_module,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let campaign_objects = overlay_pipeline(
            "Instanced campaign structures",
            "object_vertex",
            "object_fragment",
            None,
        );
        let hud_pipeline = overlay_pipeline(
            "Screen text and controls",
            "vertex",
            "hud_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let ui_world_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Selection and layer UI instances"),
            size: (UI_QUAD_CAPACITY * std::mem::size_of::<hud::UiQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ui_world_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("World UI rectangles"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_module,
                entry_point: Some("ui_world_vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_module,
                entry_point: Some("ui_world_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!("GPU pipeline validation: {error}"));
        }
        let generated_ms = started.elapsed().as_secs_f64() * 1000.;
        let mut gpu = Self {
            instance,
            adapter,
            device,
            queue,
            info,
            params,
            buffers,
            compute_group,
            render_group,
            pipelines,
            render,
            troops,
            campaign_objects,
            hud_pipeline,
            hud_texture,
            hud_model: None,
            hud_regions: Vec::new(),
            hud_raster: hud::RasterCache::default(),
            hud_diagnostics: HudDiagnostics::default(),
            hud_cpu_timings: crate::metrics::FrameTimings::new(2048),
            ui_world_pipeline,
            ui_world_buffer,
            ui_world_count: 0,
            campaign_mode,
            ground_texture,
            format,
            generated_ms,
            last_count: u32::MAX,
            last_time: -1.,
            terrain_revision: terrain.revision(),
            vision_dirty: true,
            mask_geometry_dirty: true,
        };
        // Startup buffer/texture already contain these scene edits.
        terrain.take_dirty_cells();
        terrain.take_dirty_pixels();
        if !campaign_mode {
            gpu.initialize_masks();
        }
        Ok((gpu, surface, terrain))
    }
    /// Upload display copies only. CPU campaign state remains authoritative;
    /// none of the benchmark movement/binning/vision kernels run afterward.
    pub fn upload_campaign_agents(&mut self, agents: &[Agent]) -> Result<u64, String> {
        validate_campaign_agents(agents)?;
        self.campaign_mode = true;
        self.params.counts[2] = 1;
        self.params.sim[0] = agents.len() as u32;
        let data = bytemuck::cast_slice::<Agent, u8>(agents);
        if !data.is_empty() {
            self.queue.write_buffer(&self.buffers[3], 0, data);
        }
        Ok(data.len() as u64)
    }

    /// Structures are drawn as instances, so cell.object is not a rendering
    /// requirement. Collision and sight footprints remain owned by the CPU.
    pub fn upload_campaign_objects(
        &mut self,
        objects: &[Object],
        physical_count: usize,
    ) -> Result<u64, String> {
        validate_campaign_objects(objects, physical_count)?;
        self.campaign_mode = true;
        self.params.counts[2] = 1;
        self.params.counts[0] = physical_count as u32;
        self.params.counts[1] = (objects.len() - physical_count) as u32;
        let data = bytemuck::cast_slice::<Object, u8>(objects);
        if !data.is_empty() {
            self.queue.write_buffer(&self.buffers[5], 0, data);
        }
        Ok(data.len() as u64)
    }

    /// Visible/explored/coverage masks are already filtered for one commander.
    /// Radar contacts are separate stale display records, not live hidden units.
    pub fn upload_campaign_intel(&mut self, masks: &[u32]) -> Result<u64, String> {
        if masks.len() != CAMPAIGN_MASK_CELLS || masks.iter().any(|m| m & !15 != 0) {
            return Err("Campaign visibility requires15000 masks with bits0..3 only".into());
        }
        self.campaign_mode = true;
        self.params.counts[2] = 1;
        let data = bytemuck::cast_slice::<u32, u8>(masks);
        self.queue.write_buffer(&self.buffers[6], 240000 * 4, data);
        Ok(data.len() as u64)
    }

    pub fn upload_campaign_masks(&mut self, masks: &[u32]) -> Result<u64, String> {
        self.upload_campaign_intel(masks)
    }

    pub fn set_ui_world_overlays(
        &mut self,
        model: &hud::HudModel,
        width: u32,
        height: u32,
    ) -> Result<u64, String> {
        let quads = hud::world_quads(model, width, height);
        if quads.len() > UI_QUAD_CAPACITY {
            return Err("UI instance budget exceeded; gameplay counts were not changed".into());
        }
        self.ui_world_count = quads.len() as u32;
        self.hud_diagnostics.world_quads = quads.len();
        let bytes = bytemuck::cast_slice::<hud::UiQuad, u8>(&quads);
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.ui_world_buffer, 0, bytes);
        }
        Ok(bytes.len() as u64)
    }
    pub fn set_hud(
        &mut self,
        model: &hud::HudModel,
        width: u32,
        height: u32,
    ) -> Result<u64, String> {
        let started = web_time::Instant::now();
        if width == 0 || height == 0 {
            return Err("HUD viewport must have nonzero dimensions".into());
        }
        let (w, h) = hud::canvas_size(width, height);
        if self
            .hud_model
            .as_ref()
            .is_some_and(|(old, ow, oh)| old == model && *ow == w && *oh == h)
        {
            self.hud_diagnostics.model_cache_hits += 1;
            self.hud_diagnostics.last_cpu_ms = started.elapsed().as_secs_f64() * 1000.;
            self.hud_diagnostics.last_upload_bytes = 0;
            self.hud_diagnostics.last_dirty_rects = 0;
            self.hud_cpu_timings
                .record_seconds(started.elapsed().as_secs_f64());
            return Ok(0);
        }
        let mut uploaded = self.set_ui_world_overlays(model, width, height)?;
        let image = self.hud_raster.draw(model, width, height);
        let stride = w as usize * 4;
        for r in &image.dirty {
            let offset = r.y as usize * stride + r.x as usize * 4;
            let length = (r.height as usize - 1) * stride + r.width as usize * 4;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.hud_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: r.x,
                        y: r.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &image.rgba[offset..offset + length],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(r.height),
                },
                wgpu::Extent3d {
                    width: r.width,
                    height: r.height,
                    depth_or_array_layers: 1,
                },
            );
            uploaded += r.width as u64 * r.height as u64 * 4;
        }
        self.hud_regions = image.regions;
        self.hud_diagnostics.last_dirty_rects = image.dirty.len();
        self.hud_diagnostics.layers_redrawn += image.layers_redrawn;
        self.hud_diagnostics.layers_reused += image.layers_reused;
        self.hud_model = Some((model.clone(), w, h));
        self.hud_diagnostics.updates += 1;
        self.hud_diagnostics.last_upload_bytes = uploaded;
        self.hud_diagnostics.total_upload_bytes += uploaded;
        self.hud_diagnostics.last_cpu_ms = started.elapsed().as_secs_f64() * 1000.;
        self.hud_cpu_timings
            .record_seconds(started.elapsed().as_secs_f64());
        Ok(uploaded)
    }
    pub fn hud_diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"counters":self.hud_diagnostics,"cpu_composition_samples":self.hud_cpu_timings.summary(),"glyph_source":"bundled licensed Rajdhani and IBM Plex Sans glyph atlases","rgba_texture_capacity_bytes":hud::MAX_WIDTH as u64*hud::MAX_HEIGHT as u64*4,"quad_capacity":UI_QUAD_CAPACITY,"gpu_ui_execution_ms":null,"budget_ms_target":{"cpu":1.,"gpu":1.},"scope":"CPU layout/raster/upload submission; includes explicit capture variants; no GPU timestamp measurement"})
    }
    pub fn clear_hud(&mut self) {
        self.hud_model = None;
        self.hud_regions.clear();
        self.ui_world_count = 0;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture_ui_evidence(
        &mut self,
        frames: &[UiEvidenceFrame],
        prefix: &str,
    ) -> Result<Vec<String>, String> {
        self.capture_ui_evidence_in(frames, std::path::Path::new(""), prefix)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture_ui_evidence_in(
        &mut self,
        frames: &[UiEvidenceFrame],
        directory: &std::path::Path,
        prefix: &str,
    ) -> Result<Vec<String>, String> {
        validate_ui_evidence_prefix(prefix)?;
        let old_params = self.params;
        let old_hud = self.hud_model.clone();
        let result = (|| {
            let mut files = Vec::new();
            for item in frames {
                if item.width == 0
                    || item.height == 0
                    || item.width > hud::MAX_WIDTH
                    || item.height > hud::MAX_HEIGHT
                    || !item
                        .label
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    return Err("Invalid UI evidence frame".into());
                }
                self.params.screen[0] = item.width as f32;
                self.params.screen[1] = item.height as f32;
                self.set_hud(&item.model, item.width, item.height)?;
                let target = self.target(item.width, item.height);
                self.render(&target.create_view(&Default::default()), 0);
                let path =
                    ui_evidence_path(directory, prefix, item.width, item.height, &item.label)?;
                self.capture(&target, &path)?;
                let path = path.to_string_lossy().into_owned();
                crate::logging::log("CAPTURE", &path);
                files.push(path);
            }
            Ok::<_, String>(files)
        })();
        self.params = old_params;
        let restore = if let Some((model, w, h)) = old_hud {
            self.set_hud(&model, w, h).map(|_| ())
        } else {
            self.clear_hud();
            Ok(())
        };
        match (result, restore) {
            (Ok(files), Ok(())) => Ok(files),
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(format!("Restoring UI after capture: {e}")),
        }
    }

    /// CPU data is authoritative. Upload only changed rows/rectangles; all
    /// commands precede the next simulation/render submission on this queue.
    pub fn upload_campaign_terrain(&mut self, c: &mut crate::campaign::Campaign) -> Result<u64,String> {
        self.params.counts[3]=u32::from(c.data.visual_memory.is_some());
        if c.data.visual_memory.is_none() { return Ok(self.upload_terrain(&mut c.terrain)); }
        let mut dirty=vec![false;15_000];
        for range in c.terrain.take_dirty_cells() { for cell in range {
            dirty[(cell/CW/4)*150+(cell%CW/4)]=true;
        }}
        for r in c.terrain.take_dirty_pixels() {
            for y in r.y as usize/16..=(r.y as usize+r.height as usize-1)/16 {
                for x in r.x as usize/16..=(r.x as usize+r.width as usize-1)/16 { dirty[y*150+x]=true; }
            }
        }
        let objects=c.draw_objects()?;
        let masks=c.intel().to_vec();
        let memory=c.data.visual_memory.as_mut().ok_or("Missing presentation memory")?;
        let spans=memory.refresh(&c.terrain,&masks,&dirty,&objects);
        let mut bytes=0;
        let stride=WW/2*4;
        for (start,end) in spans {
            let x=start%150; let y=start/150; let width=end-start;
            for row in 0..4 {
                let offset=(y*4+row)*CW+x*4;
                let data=bytemuck::cast_slice::<Cell,u8>(&memory.cells[offset..offset+width*4]);
                self.queue.write_buffer(&self.buffers[1],(offset*std::mem::size_of::<Cell>()) as u64,data);
                bytes+=data.len() as u64;
            }
            let offset=(y*16*(WW/2)+x*16)*4;
            let length=15*stride+width*16*4;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture:&self.ground_texture,mip_level:0,
                    origin:wgpu::Origin3d{x:(x*16) as u32,y:(y*16) as u32,z:0},aspect:wgpu::TextureAspect::All },
                &memory.rgba[offset..offset+length],
                wgpu::TexelCopyBufferLayout { offset:0,bytes_per_row:Some(stride as u32),rows_per_image:Some(16) },
                wgpu::Extent3d {width:(width*16) as u32,height:16,depth_or_array_layers:1});
            bytes+=(width*16*16*4) as u64;
        }
        Ok(bytes)
    }
    pub fn upload_terrain(&mut self, terrain: &mut Terrain) -> u64 {
        let mut bytes = 0u64;
        for range in terrain.take_dirty_cells() {
            let data = bytemuck::cast_slice::<Cell, u8>(&terrain.cells[range.clone()]);
            self.queue.write_buffer(
                &self.buffers[1],
                (range.start * std::mem::size_of::<Cell>()) as u64,
                data,
            );
            bytes += data.len() as u64;
        }
        let stride = WW / 2 * 4;
        for rect in terrain.take_dirty_pixels() {
            let offset = rect.y as usize * stride + rect.x as usize * 4;
            let length = (rect.height as usize - 1) * stride + rect.width as usize * 4;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.ground_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: rect.x,
                        y: rect.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &terrain.rgba[offset..offset + length],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride as u32),
                    rows_per_image: Some(rect.height),
                },
                wgpu::Extent3d {
                    width: rect.width,
                    height: rect.height,
                    depth_or_array_layers: 1,
                },
            );
            bytes += rect.width as u64 * rect.height as u64 * 4;
        }
        if self.terrain_revision != terrain.revision() {
            self.terrain_revision = terrain.revision();
            self.mask_geometry_dirty = true;
        }
        bytes
    }
    pub fn initialize_masks(&mut self) {
        if self.campaign_mode {
            return;
        }
        self.vision_dirty = true;
        self.mask_geometry_dirty = false;
        let mut e = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Cached emitter geometry"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &self.compute_group, &[]);
            pass.set_pipeline(&self.pipelines[3]);
            pass.dispatch_workgroups((self.params.counts[1] * 15000).div_ceil(64), 1, 1);
        }
        self.queue.submit([e.finish()]);
    }
    pub fn render(&mut self, view: &wgpu::TextureView, steps: u32) {
        self.params.sim[3] = steps;
        self.queue
            .write_buffer(&self.buffers[0], 0, bytemuck::bytes_of(&self.params));
        let sight_enabled = self.params.misc[1] > 0.5 || self.params.misc[2] > 0.5;
        if self.mask_geometry_dirty && sight_enabled && !self.campaign_mode {
            self.initialize_masks();
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let movement_changed =
            (steps > 0 && self.params.sim[0] > 0) || self.last_count != self.params.sim[0];
        for (pipeline, count) in [
            (0, 60000),
            (1, self.params.sim[0]),
            (2, self.params.sim[0]),
            (4, 15000),
        ] {
            if self.campaign_mode {
                continue;
            }
            if pipeline == 4 && !sight_enabled {
                continue;
            }
            if pipeline != 4 && !movement_changed {
                continue;
            }
            if pipeline == 4
                && self.last_time == self.params.screen[3]
                && !movement_changed
                && !self.vision_dirty
            {
                continue;
            }
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GPU simulation"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &self.compute_group, &[]);
            pass.set_pipeline(&self.pipelines[pipeline]);
            pass.dispatch_workgroups(count.div_ceil(if pipeline == 4 { 64 } else { 128 }), 1, 1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Pixel ground and grown forms"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render);
            pass.set_bind_group(0, &self.render_group, &[]);
            pass.draw(0..3, 0..1);
            if self.campaign_mode {
                pass.set_pipeline(&self.campaign_objects);
                pass.draw(0..6, 0..self.params.counts[0]);
            }
            pass.set_pipeline(&self.troops);
            pass.draw(0..6, 0..self.params.sim[0]);
            if self.ui_world_count > 0 {
                pass.set_pipeline(&self.ui_world_pipeline);
                pass.set_vertex_buffer(0, self.ui_world_buffer.slice(..));
                pass.draw(0..6, 0..self.ui_world_count);
            }
            if let Some((_, canvas_width, canvas_height)) = &self.hud_model {
                pass.set_pipeline(&self.hud_pipeline);
                let width = self.params.screen[0] as u32;
                let height = self.params.screen[1] as u32;
                for r in &self.hud_regions {
                    let x = (r.x as f64 * width as f64 / *canvas_width as f64).floor() as u32;
                    let y = (r.y as f64 * height as f64 / *canvas_height as f64).floor() as u32;
                    let right = ((r.x + r.width) as f64 * width as f64 / *canvas_width as f64)
                        .ceil() as u32;
                    let bottom = ((r.y + r.height) as f64 * height as f64 / *canvas_height as f64)
                        .ceil() as u32;
                    if right > x && bottom > y && x < width && y < height {
                        pass.set_scissor_rect(x, y, right.min(width) - x, bottom.min(height) - y);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.last_count = self.params.sim[0];
        self.last_time = self.params.screen[3];
        self.vision_dirty = false;
    }
    pub fn target(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Capture target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_buffer(&self, index: usize, bytes: u64) -> Vec<u8> {
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("On-demand diagnostic readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_buffer_to_buffer(&self.buffers[index], 0, &read, 0, bytes);
        self.queue.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let data = read.slice(..).get_mapped_range().to_vec();
        read.unmap();
        data
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture(&self, texture: &wgpu::Texture, path: &std::path::Path) -> Result<(), String> {
        let width = self.params.screen[0] as u32;
        let height = self.params.screen[1] as u32;
        let pitch = (width * 4).div_ceil(256) * 256;
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Capture readback"),
            size: pitch as u64 * height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([e.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv().unwrap().map_err(|e| e.to_string())?;
        let data = read.slice(..).get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in data.chunks(pitch as usize) {
            pixels.extend_from_slice(&row[..width as usize * 4]);
        }
        if self.format == wgpu::TextureFormat::Bgra8Unorm {
            for pixel in pixels.chunks_mut(4) {
                pixel.swap(0, 2);
            }
        }
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&pixels)
            .map_err(|e| e.to_string())?;
        drop(data);
        read.unmap();
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_ui_evidence_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() || prefix.contains('/') || prefix.contains('\\') || prefix.contains("..") {
        return Err("Invalid UI evidence prefix".into());
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
fn ui_evidence_path(
    directory: &std::path::Path,
    prefix: &str,
    width: u32,
    height: u32,
    label: &str,
) -> Result<std::path::PathBuf, String> {
    validate_ui_evidence_prefix(prefix)?;
    if width == 0
        || height == 0
        || width > hud::MAX_WIDTH
        || height > hud::MAX_HEIGHT
        || label.is_empty()
        || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Invalid UI evidence frame".into());
    }
    Ok(directory.join(format!("{prefix}-ui-{width}x{height}-{label}.png")))
}

#[cfg(test)]
mod campaign_render_tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn nested_benchmark_capture_directory_is_separate_from_safe_basename() {
        let parent = std::path::Path::new("check-results/run-22544/benchmark");
        let prefix = "window-speed-ramp-10800-1793403026109";
        assert_eq!(
            ui_evidence_path(parent, prefix, 1920, 1080, "calm").unwrap(),
            parent.join(format!("{prefix}-ui-1920x1080-calm.png"))
        );
        for bad in ["", "../escape", "x/y", "x\\y", "x..y"] {
            assert!(ui_evidence_path(parent, bad, 1920, 1080, "calm").is_err());
        }
        assert!(ui_evidence_path(parent, prefix, 1920, 1080, "../escape").is_err());
    }

    #[test]
    fn every_construction_material_resolves_both_teams_all_variants_to_visible_atlas_pixels() {
        use crate::construction::BuildKind;
        prewarm_campaign_materials().unwrap();
        let (pixels, width, height) = decode_png(include_bytes!("../assets/pixel-atlas.png"));
        assert_eq!((width, height), (1024, 1024));
        let mut names = std::collections::BTreeSet::new();
        for kind in BuildKind::ALL {
            assert!(
                names.insert(kind.name()),
                "Duplicate construction tool {kind:?}"
            );
            for team in 0..2 {
                for variant in 0..4 {
                    let object =
                        grown_object(kind.atlas_kind(), team, [100., 100.], 32., 4., 0.5, variant)
                            .unwrap_or_else(|e| {
                                panic!("{kind:?}, team {team}, variant {variant}: {e}")
                            });
                    let [x, y, w, h] = object.uv;
                    assert_eq!((w, h), (64, 64));
                    let visible = (y..y + h).any(|yy| {
                        (x..x + w).any(|xx| pixels[((yy * width + xx) * 4 + 3) as usize] >= 128)
                    });
                    assert!(
                        visible,
                        "Empty crop for {kind:?}, team {team}, variant {variant}"
                    );
                }
            }
        }
        assert_eq!(names.len(), 19);
        assert_eq!(BuildKind::Wall.atlas_kind(), "wall");
        assert_eq!(BuildKind::Turret.atlas_kind(), "turret");
        assert_eq!(BuildKind::Dam.atlas_kind(), "wall");
    }

    #[test]
    fn known_grown_materials_are_reused_and_invalid_completion_is_rejected() {
        for name in [
            "ship", "depot", "mine", "mill", "barracks", "camera", "radar", "wall", "trench",
        ] {
            for team in 0..2 {
                let object = grown_object(name, team, [100., 100.], 32., 4., 0.5, 19).unwrap();
                assert_eq!(object.complete, 0.5);
                assert_eq!(object.uv[2..], [64, 64]);
            }
        }
        assert!(grown_object("invented", 0, [100., 100.], 32., 4., 1., 0).is_err());
        assert!(grown_object("ship", 0, [100., 100.], 32., 4., 1.1, 0).is_err());
        assert!(grown_object("ship", 0, [100., 100.], 0., 4., 1., 0).is_err());
    }

    #[test]
    fn display_records_validate_material_families_and_never_reduce_counts() {
        let mut agent = Agent {
            pos: [100., 100.],
            facing: 0.,
            id: 77,
            goal: 0,
            team: 0,
            role: 1,
            next: -1,
        };
        assert!(validate_campaign_agents(&[agent]).is_ok());
        agent.role = 4;
        agent.team = 3;
        assert!(validate_campaign_agents(&[agent]).is_ok());
        agent.team = 4;
        assert!(validate_campaign_agents(&[agent]).is_err());
        agent.role = 5;
        agent.team = 0;
        assert!(validate_campaign_agents(&[agent]).is_ok());
        agent.pos[0] = f32::NAN;
        assert!(validate_campaign_agents(&[agent]).is_err());
        assert!(validate_campaign_agents(&vec![Agent::zeroed(); CAPACITY + 1]).is_err());
        agent.pos[0] = 100.;
        agent.role = 4;
        agent.team = crate::natural_colors::NaturalColors::new(78421).fauna_appearance(19);
        assert!(validate_campaign_agents(&[agent]).is_ok());
        agent.role = 1;
        assert!(validate_campaign_agents(&[agent]).is_err());
        agent.role = 4;
        agent.team = 0x8400_0000;
        assert!(validate_campaign_agents(&[agent]).is_err());
    }
}
