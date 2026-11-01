struct Params{view:vec4<f32>,screen:vec4<f32>,sim:vec4<u32>,world:vec4<u32>,misc:vec4<f32>,counts:vec4<u32>}
struct Cell{height:f32,cover:f32,water:u32,flags:u32,object:u32,soil:u32,pad:vec2<u32>}
struct Agent{pos:vec2<f32>,facing:f32,id:u32,goal:u32,team:u32,role:u32,next:i32}
struct Object{pos:vec2<f32>,size:f32,kind:u32,uv:vec4<u32>,team:u32,height:f32,complete:f32,seed:u32}
@group(0)@binding(0)var<uniform> p:Params;
@group(0)@binding(1)var<storage,read> cells:array<Cell>;
@group(0)@binding(2)var<storage,read> flows:array<vec2<f32>>;
@group(0)@binding(3)var<storage,read_write> agents:array<Agent>;
@group(0)@binding(4)var<storage,read_write> heads:array<atomic<i32>>;
@group(0)@binding(5)var<storage,read> objects:array<Object>;
@group(0)@binding(6)var<storage,read_write> intel:array<u32>;
@group(0)@binding(7)var ground:texture_2d<f32>;
@group(0)@binding(8)var atlas:texture_2d<f32>;
fn world_cell(pos:vec2<f32>)->u32{let a=vec2<u32>(clamp(pos/8.,vec2(0.),vec2(599.,399.)));return a.y*600u+a.x;}
fn nav_cell(pos:vec2<f32>)->u32{let a=vec2<u32>(clamp(pos/16.,vec2(0.),vec2(299.,199.)));return a.y*300u+a.x;}
fn passable(pos:vec2<f32>)->bool{let c=cells[world_cell(pos)];let wet=select(c.water>0u,c.water>=300u,(c.flags&2u)!=0u);return(!wet||(c.flags&1u)!=0u)&&c.object==0u;}
@compute @workgroup_size(128) fn clear(@builtin(global_invocation_id) gid:vec3<u32>){if gid.x<60000u{atomicStore(&heads[gid.x],-1);}}
@compute @workgroup_size(128) fn movement(@builtin(global_invocation_id) gid:vec3<u32>){let i=gid.x;if i>=p.sim.x{return;}var a=agents[i];let goals=array<vec2<f32>,4>(vec2(700.,1640.),vec2(1800.,1640.),vec2(3000.,1712.),vec2(4000.,1712.));
 for(var n=0u;n<p.sim.w;n++){if distance(a.pos,goals[a.goal])<32.{a.goal=(a.goal+1u)%4u;}let d=flows[a.goal*60000u+nav_cell(a.pos)];let c=cells[world_cell(a.pos)];let speed=select(max(.18,1.-c.cover/310.),2.4,(c.flags&1u)!=0u);let step=d*18.*speed*p.misc.x;let next=a.pos+step;if passable(next){a.pos=next;}else{let side=vec2(-d.y,d.x)*select(-1.,1.,a.id%2u==0u)*8.;if passable(a.pos+side){a.pos+=side*p.misc.x;}}if length(d)>.01{a.facing=atan2(d.y,d.x);}}
 a.pos=clamp(a.pos,vec2(8.),vec2(4792.,3192.));agents[i]=a;
}
@compute @workgroup_size(128) fn binning(@builtin(global_invocation_id) gid:vec3<u32>){let i=gid.x;if i>=p.sim.x{return;}let old=atomicExchange(&heads[nav_cell(agents[i].pos)],i32(i));agents[i].next=old;}
fn solid(pos:vec2<f32>,ignore:u32)->f32{let c=cells[world_cell(pos)];var top=c.height*.01;if c.object>0u&&c.object-1u!=ignore{let b=objects[c.object-1u];let uv=vec2<i32>((pos-b.pos)/b.size*64.+vec2(32.));if all(uv>=vec2(0))&&all(uv<vec2(64)){if textureLoad(atlas,vec2<i32>(b.uv.xy)+uv,0).a>.1{top+=b.height;}}}return top;}
// Static emitter masks only run after terrain/structure changes, never per troop.
@compute @workgroup_size(64) fn sensor_masks(@builtin(global_invocation_id) gid:vec3<u32>){let flat=gid.x;let emitter=flat/15000u;let cell=flat%15000u;if emitter>=p.counts.y{return;}let src=objects[p.counts.x+emitter];let destination=vec2<f32>(f32(cell%150u)*32.+16.,f32(cell/150u)*32.+16.);let dist=distance(src.pos,destination);var visible=dist<=src.size;let start=cells[world_cell(src.pos)].height*.01+src.height;let finish=cells[world_cell(destination)].height*.01+1.4;
 if visible&&dist>16.{let steps=u32(dist/8.);for(var n=1u;n<steps;n++){let at=mix(src.pos,destination,f32(n)/f32(steps));let limit=select(mix(start,finish,f32(n)/f32(steps)),start,src.kind==101u);if solid(at,src.seed)>limit+.05{visible=false;break;}}}intel[flat]=select(0u,1u,visible);
}
@compute @workgroup_size(64) fn combine_vision(@builtin(global_invocation_id) gid:vec3<u32>){let cell=gid.x;if cell>=15000u{return;}let pos=vec2<f32>(f32(cell%150u)*32.+16.,f32(cell/150u)*32.+16.);var bits=0u;for(var source=0u;source<p.counts.y;source++){if intel[source*15000u+cell]==0u{continue;}let src=objects[p.counts.x+source];if src.kind==100u{bits|=1u;}else{bits|=4u;let angle=p.screen.w/8.*6.2831853+f32(source)*.73;let delta=atan2(sin(atan2(pos.y-src.pos.y,pos.x-src.pos.x)-angle),cos(atan2(pos.y-src.pos.y,pos.x-src.pos.x)-angle));if abs(delta)<.261799{bits|=2u;}}}
 let phase=p.screen.w%180.;if phase<18.&&p.screen.w>=180.{let center=vec2(-1800.+phase/18.*8400.,1600.);if distance(pos,center)<1800.{bits|=1u;}}
 let offset=240000u+cell;let explored=intel[offset]&8u;if (bits&1u)!=0u{bits|=8u;}intel[offset]=bits|explored;
}
