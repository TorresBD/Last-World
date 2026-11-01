pub mod natural_colors;
pub mod clock;
pub mod diagnostics;
pub mod frame_profile;
pub mod climate;
pub mod gpu;
pub mod ground;
pub mod habitat;
pub mod logging;
pub mod metrics;
pub mod river;
pub mod runtime;
pub mod terrain;
pub mod water;
pub fn engine_identity() -> &'static str {
    "Bevy 0.20.1 + wgpu 27 GPU compute"
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub async fn wasm_start() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    logging::install();
    if let Err(error) = runtime::windowed(None).await {
        logging::log("FATAL", &error);
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(status) = document.get_element_by_id("native-status") {
                status.set_text_content(Some(&format!("GPU test could not start: {error}")));
            }
        }
        return Err(error.into());
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn native_command(command: &str) {
    runtime::web_command(command);
}

#[cfg(test)]
mod shader_tests {
    #[test]
    fn gpu_kernels_validate() {
        for source in [
            include_str!("../shaders/compute.wgsl"),
            include_str!("../shaders/render.wgsl"),
        ] {
            let module = naga::front::wgsl::parse_str(source).expect("WGSL parser");
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("WGSL validation");
        }
    }
}

pub mod construction;

pub mod tactics;

pub mod ecology;

pub mod campaign_support;

pub mod campaign;

pub mod play;

pub mod campaign_scenarios;

pub mod ecosystem_physics;

pub mod faction_support;

pub mod ui_interaction;

pub mod campaign_commands;
pub mod ecology_navigation;
pub mod campaign_habitats;
pub mod ecology_behavior;
pub mod campaign_benchmark;
pub mod benchmark_orders;

pub mod battle_ai;
