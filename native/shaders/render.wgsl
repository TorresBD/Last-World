struct Params{view:vec4<f32>,screen:vec4<f32>,sim:vec4<u32>,world:vec4<u32>,misc:vec4<f32>,counts:vec4<u32>}
struct Cell{height:f32,cover:f32,water:u32,flags:u32,object:u32,soil:u32,pad:vec2<u32>}
struct Agent{pos:vec2<f32>,facing:f32,id:u32,goal:u32,team:u32,role:u32,next:i32}
struct Object{pos:vec2<f32>,size:f32,kind:u32,uv:vec4<u32>,team:u32,height:f32,complete:f32,seed:u32}
@group(0)@binding(0)var<uniform> p:Params;
@group(0)@binding(1)var<storage,read> cells:array<Cell>;
@group(0)@binding(2)var<storage,read> flows:array<vec2<f32>>;
@group(0)@binding(3)var<storage,read> agents:array<Agent>;
@group(0)@binding(4)var<storage,read> heads:array<i32>;
@group(0)@binding(5)var<storage,read> objects:array<Object>;
@group(0)@binding(6)var<storage,read> intel:array<u32>;
@group(0)@binding(7)var ground:texture_2d<f32>;
@group(0)@binding(8)var atlas:texture_2d<f32>;
@group(0)@binding(9)var hud:texture_2d<f32>;
@vertex fn vertex(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{let x=f32(i&1u)*4.-1.;let y=f32(i>>1u)*4.-1.;return vec4(x,y,0.,1.);}
fn cell_at(pos:vec2<f32>)->u32{let xy=vec2<u32>(clamp(pos/8.,vec2(0.),vec2(599.,399.)));return xy.y*600u+xy.x;}
fn random(v:vec2<i32>)->f32{var n=u32(v.x)*374761393u^u32(v.y+17)*668265263u;n=(n^(n>>13u))*1274126177u;return f32(n^(n>>16u))/4294967295.;}
fn water_depth(pos:vec2<f32>,nearest:Cell)->f32{
 // Reconstruct a continuous bank within an authoritative wetted sample.
 // Dry samples never gain rendered water across a disconnected ridge.
 if nearest.water==0u{return 0.;}
 let grid=pos/8.-vec2(.5);let base=floor(grid);let f=fract(grid);
 let weights=array<f32,4>((1.-f.x)*(1.-f.y),f.x*(1.-f.y),(1.-f.x)*f.y,f.x*f.y);
 let offsets=array<vec2<f32>,4>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(1.,1.));
 var bed=0.;var head=0.;var wet_weight=0.;
 for(var i=0u;i<4u;i++){
  let c=cells[cell_at((base+offsets[i]+vec2(.5))*8.)];
  bed+=c.height*.01*weights[i];
  if (c.flags&2u)!=0u&&c.water>0u{head+=(c.height*.01+f32(c.water)*.001)*weights[i];wet_weight+=weights[i];}
 }
 if wet_weight<.001{return 0.;}
 return max(0.,head/wet_weight-bed);
}
@fragment fn fragment(@builtin(position) pixel:vec4<f32>)->@location(0)vec4<f32>{let world=floor(p.view.xy+(pixel.xy-p.screen.xy*.5)/p.screen.z);if any(world<vec2(0.))||any(world>=vec2(4800.,3200.)){return vec4(.035,.065,.045,1.);}let cell=cells[cell_at(world)];var color=textureLoad(ground,vec2<i32>(world*.5),0).rgb;let grain=random(vec2<i32>(world/2.))*.08-.04;
 let vision_cell=u32(world.y/32.)*150u+u32(world.x/32.);let mask=intel[240000u+vision_cell];let visible=p.misc.y<.5||(mask&1u)!=0u;
 // Water is derived from finite CPU storage. Only connected wetted samples can
 // draw water; animated grain follows actual last-tick transfer direction.
 if (cell.flags&2u)!=0u&&cell.water>0u{
  let velocity=vec2(bitcast<f32>(cell.pad.x),bitcast<f32>(cell.pad.y));
  let moving_grain=random(vec2<i32>(floor((world-velocity*(select(0.,p.screen.w,visible)%512.))*.5)))*.055-.0275;
  let depth=water_depth(world,cell);
  let water_color=mix(vec3(.30,.205,.115),vec3(.085,.05,.027),clamp(depth/2.,0.,1.))+moving_grain*.5;
  color=mix(color,water_color,smoothstep(0.,.08,depth)*.96);
 }
 if (cell.flags&1u)!=0u{color=vec3(.412,.431,.427)+grain;if cell.water>0u{color=vec3(.443,.412,.329)+grain;}}

 if !visible{
  if p.counts.z!=0u {
   // Campaign buffers contain the initial survey / last optical snapshot.
   if p.counts.w!=0u{color*=.72;}else{color=select(vec3(.035,.071,.055),vec3(.055,.085,.067),(mask&8u)!=0u);}
  }else{color=mix(vec3(.035,.071,.055),color,select(.12,.42,(mask&8u)!=0u));}
 }
 if p.counts.z==0u&&cell.object>0u&&cell.object<=p.counts.x{let b=objects[cell.object-1u];let xy=vec2<i32>((world-b.pos)/b.size*64.+vec2(32.));if visible&&all(xy>=vec2(0))&&all(xy<vec2(64)){let paint=textureLoad(atlas,vec2<i32>(b.uv.xy)+xy,0);color=mix(color,paint.rgb,paint.a);}}
 if p.misc.z>.5&&(mask&4u)!=0u{color=mix(color,vec3(.53,.69,.57),.14);}
 // No continuous lighting pass. This branch executes only during a 440 ms flash.
 if visible&&p.misc.w>=0.&&p.misc.w<.44{let center=select(p.view.xy,p.view.zw,p.counts.z!=0u);let distance_to_blast=distance(world,center);let falloff=max(0.,1.-distance_to_blast/(110.+random(vec2<i32>(world/6.))*45.));if falloff>0.{var shadow=false;let origin=cells[cell_at(center)].height*.01+8.;for(var n=1u;n<16u;n++){let at=mix(center,world,f32(n)/16.);let c=cells[cell_at(at)];if c.object>0u&&objects[c.object-1u].height+c.height*.01>origin{shadow=true;break;}}let light=falloff*(1.-p.misc.w/.44)*select(.7,.04,shadow);color+=vec3(1.,.61,.25)*light;}}
 return vec4(color,1.);
}

// One instance per troop reads GPU-owned state directly. No per-ground-pixel
// crowd traversal: the rasterizer visits only each visible material footprint.
struct TroopVertex {
 @builtin(position) clip: vec4<f32>,
 @location(0) uv: vec2<f32>,
 @location(1) world: vec2<f32>,
 @location(2) @interpolate(flat) tile: vec2<u32>,
 @location(3) @interpolate(flat) mask: u32,
 @location(4) @interpolate(flat) render_kind: u32,
 @location(5) @interpolate(flat) coat: vec3<f32>,
}
@vertex fn troop_vertex(@builtin(vertex_index) v:u32,@builtin(instance_index) id:u32)->TroopVertex {
 let a=agents[id];
 var out:TroopVertex;
 out.render_kind=a.role;
 out.coat=vec3(-1.);
 let vc=u32(clamp(a.pos.y/32.,0.,99.))*150u+u32(clamp(a.pos.x/32.,0.,149.));
 out.mask=intel[240000u+vc];
 let corners=array<vec2<f32>,6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(-1.,1.),vec2(-1.,1.),vec2(1.,-1.),vec2(1.,1.));
 let footprint_scale=select(1.,.5,p.counts.z!=0u&&a.role<=3u);
 let local=corners[v]*5.714286*footprint_scale;
 let cs=cos(a.facing);let sn=sin(a.facing);
 out.world=a.pos+vec2(local.x*cs-local.y*sn,local.x*sn+local.y*cs);
 let screen=(out.world-p.view.xy)*p.screen.z/(p.screen.xy*.5);
 out.clip=vec4(screen.x,-screen.y,0.,1.);
 let center=(a.pos-p.view.xy)*p.screen.z/(p.screen.xy*.5);
 let extent=vec2(8.1)*footprint_scale*p.screen.z/(p.screen.xy*.5);
 let hidden=select(p.misc.y>.5&&(out.mask&1u)==0u,(out.mask&4u)==0u||(out.mask&1u)!=0u||p.misc.y<.5,a.role==5u);
 if hidden||any(abs(center)>vec2(1.)+extent){
  // Every vertex agrees: fully hidden instances collapse to an empty triangle.
  out.clip=vec4(-2.,-2.,0.,1.);
 }
 out.uv=(corners[v]+vec2(1.))*8.;
 let pose=u32(p.screen.w*5.+f32(a.id))%4u;
 var slot=a.team*128u+a.role*32u+(a.id%8u)*4u+pose;
 if a.role==4u{
  var family=a.team;
  if (a.team&0x80000000u)!=0u {family=(a.team>>24u)&3u;out.coat=unpack4x8unorm(a.team).xyz;}
  slot=256u+family*16u+(a.id%4u)*4u+pose;
 }
 if a.role==5u{slot=0u;}
 out.tile=vec2(slot%64u*16u,slot/64u*16u);
 return out;
}
@fragment fn troop_fragment(in:TroopVertex)->@location(0)vec4<f32>{
 if in.render_kind==5u {
  // Commander radar UI: no faction color, anatomy, health or live hidden data.
  let radius=distance(in.uv,vec2(8.));
  if radius<5.5||radius>7.{discard;}
  return vec4(.58,.64,.58,1.);
 }
 let xy=vec2<u32>(clamp(floor(in.uv),vec2(0.),vec2(15.)));
 let paint=textureLoad(atlas,vec2<i32>(in.tile+xy),0);
 if paint.a<.5{discard;}
 var color=paint.rgb;
 if in.render_kind==4u&&in.coat.x>=0. {
  let shade=.35+dot(paint.rgb,vec3(1./3.))*1.15;
  color=clamp(in.coat*shade,vec3(0.),vec3(1.));
 }
 if p.misc.z>.5&&(in.mask&4u)!=0u{color=mix(color,vec3(.53,.69,.57),.14);}
 if p.misc.w>=0.&&p.misc.w<.44{
  let light=max(0.,1.-distance(in.world,select(p.view.xy,p.view.zw,p.counts.z!=0u))/140.)*(1.-p.misc.w/.44);
  color+=vec3(1.,.61,.25)*light*.7;
 }
 return vec4(color,1.);
}


// CPU campaign structures reuse the same noise-grown atlas on the same device.
// Their visibility is tested at each fragment rather than revealing a whole
// building when its center happens to be visible.
struct ObjectVertex {
 @builtin(position) clip:vec4<f32>,
 @location(0) uv:vec2<f32>,
 @location(1) world:vec2<f32>,
 @location(2) @interpolate(flat) origin:vec2<u32>,
 @location(3) @interpolate(flat) extent:vec2<u32>,
 @location(4) @interpolate(flat) completion:f32,
 @location(5) @interpolate(flat) memory_tile:u32,
}
@vertex fn object_vertex(@builtin(vertex_index) v:u32,@builtin(instance_index) id:u32)->ObjectVertex {
 let b=objects[id];
 let corners=array<vec2<f32>,6>(vec2(-1.,-1.),vec2(1.,-1.),vec2(-1.,1.),vec2(-1.,1.),vec2(1.,-1.),vec2(1.,1.));
 var out:ObjectVertex;
 out.world=b.pos+corners[v]*b.size*.5;
 let screen=(out.world-p.view.xy)*p.screen.z/(p.screen.xy*.5);
 out.clip=vec4(screen.x,-screen.y,0.,1.);
 out.uv=(corners[v]+vec2(1.))*.5;
 out.origin=b.uv.xy;
 out.extent=b.uv.zw;
 out.completion=b.complete;
 out.memory_tile=b.kind;
 return out;
}
@fragment fn object_fragment(in:ObjectVertex)->@location(0)vec4<f32>{
 if any(in.world<vec2(0.))||any(in.world>=vec2(4800.,3200.)){discard;}
 let vc=u32(in.world.y/32.)*150u+u32(in.world.x/32.);
 if (in.memory_tile&0x80000000u)!=0u {
  if vc!=(in.memory_tile&0x7fffffffu){discard;}
 }else if p.misc.y>.5&&(intel[240000u+vc]&1u)==0u{discard;}
 let xy=min(vec2<u32>(floor(in.uv*vec2<f32>(in.extent))),in.extent-vec2(1u));
 let paint=textureLoad(atlas,vec2<i32>(in.origin+xy),0);
 if paint.a<.5{discard;}
 if in.completion<1.&&random(vec2<i32>(in.world))>max(.12,in.completion){discard;}
 var color=paint.rgb;
 if in.completion<1.{color=mix(vec3(.28,.39,.32),color,.35+.65*in.completion);}
 return vec4(color,1.);
}
@fragment fn hud_fragment(@builtin(position) pixel:vec4<f32>)->@location(0)vec4<f32>{
 let canvas=min(p.screen.xy,vec2(4096.,2304.));
 let xy=vec2<i32>(floor(pixel.xy*canvas/p.screen.xy));
 return textureLoad(hud,xy,0);
}

struct UiWorldVertex {@builtin(position) clip:vec4<f32>,@location(0) color:vec4<f32>}
@vertex fn ui_world_vertex(@builtin(vertex_index) id:u32,@location(0) rectangle:vec4<f32>,@location(1) color:vec4<f32>)->UiWorldVertex {
 let corners=array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(0.,1.),vec2(1.,0.),vec2(1.,1.));
 let point=rectangle.xy+corners[id]*rectangle.zw;let canvas=min(p.screen.xy,vec2(4096.,2304.));let screen=point/canvas*2.-vec2(1.);var out:UiWorldVertex;out.clip=vec4(screen.x,-screen.y,0.,1.);out.color=color;return out;
}
@fragment fn ui_world_fragment(in:UiWorldVertex)->@location(0)vec4<f32>{return in.color;}
