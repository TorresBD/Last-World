export const WORLD_W=4800, WORLD_H=3200, CELL=8, W=WORLD_W/CELL, H=WORLD_H/CELL, N=W*H;
export const NAV=16, NW=WORLD_W/NAV, NH=WORLD_H/NAV, NN=NW*NH, CHUNK=128;
export const DT=1/20, SAVE_VERSION=2, INITIAL_CAPACITY=2048;
export const RECON={cell:32,satelliteInterval:180,satelliteOffset:90,satelliteDuration:18,satelliteRadius:1800,satelliteLead:5,terrainCacheTiles:192,radarPeriod:8,radarBeam:Math.PI/6,contactLife:9.5,battery:240,rechargeSeconds:20,cameraCost:[140,0,25]};
export const TYPES={
 ship:{label:'Crash ship',short:'HQ',size:80,hp:2400,cost:[0,0],work:0,sight:520,height:8,desc:'Command core, stockpile, and 360° landing cameras. Camera troopers recharge here.'},
 depot:{label:'Supply depot',short:'D',size:36,hp:450,cost:[35,55],work:8,desc:'A local drop-off and construction supply point. Haulers rebalance stock.'},
 mine:{label:'Ore mine',short:'M',size:32,hp:350,cost:[55,35],work:12,desc:'Assigns workers to a nearby finite ore deposit. Must be within 100 m of ore.'},
 mill:{label:'Timber camp',short:'T',size:32,hp:300,cost:[30,40],work:9,desc:'Cuts nearby jungle for timber. Every harvested tree counts against your damage.'},
 farm:{label:'Hydroponics',short:'F',size:40,hp:300,cost:[50,45],work:12,desc:'Workers grow provisions without clearing more jungle. Consumes grid power.'},
 solar:{label:'Solar array',short:'P',size:40,hp:230,cost:[45,20],work:8,desc:'Adds 8 electrical power for farms, defenses, cameras and radar.'},
 barracks:{label:'Barracks',short:'B',size:44,hp:600,cost:[100,90],work:18,desc:'Recruits infantry using metal and provisions. Each soldier consumes provisions.'},
 turret:{label:'Sentry',short:'S',size:24,hp:400,cost:[65,35],work:12,range:210,desc:'Automatic defensive fire. Uses metal ammunition and power.'},
 mortar:{label:'Mortar',short:'A',size:32,hp:250,cost:[120,65],work:20,range:600,desc:'Long-range shells scar the ground and kill wildlife. High collateral damage.'},
 road:{label:'Road',short:'═',size:16,hp:100,cost:[1,2],work:0.65,desc:'Draw a route. Workers deliver material and build each section. Bridges form over rivers.'},
 trench:{label:'Trench',short:'∪',size:16,hp:180,cost:[0,2],work:1.1,desc:'Draw a defensive line. Excavates the actual ground; reduces incoming damage.'},
 wall:{label:'Barrier',short:'▥',size:16,hp:300,cost:[3,2],work:1.5,desc:'Draw a defensive barrier. Blocks pathfinding until destroyed.'},
 restore:{label:'Restore habitat',short:'+',size:48,hp:100,cost:[15,25],work:12,desc:'Restore damaged native vegetation. Repairs world integrity; your historical damage remains.'},
 camera:{label:'Camera pole',short:'C',size:16,hp:200,cost:[60,35],work:10,sight:420,height:8,power:1,desc:'An 8 m camera pole with 360° video coverage. Terrain, solid structures and dense canopy can obscure it.'},
 tower:{label:'Camera tower',short:'V',size:32,hp:350,cost:[150,100],work:18,sight:700,height:16,power:2,desc:'Elevated, long-range video. Greater height improves the view over surrounding obstacles.'},
 mountedCamera:{label:'Mounted camera',short:'c',size:8,hp:100,cost:[30,12],work:5,sight:340,height:.8,power:.5,fov:Math.PI*.75,desc:'Snaps to the outside of a completed friendly wall or building. Watches a 135° outward cone.'},
 radar:{label:'Radar mast',short:'R',size:32,hp:280,cost:[140,80],work:16,radar:950,height:6,power:4,desc:'A 6 m mast with an 8-second rotating sweep. Shaded coverage shows gaps behind taller solids. The rotating sweep reports anonymous, approximate dots outside video.'}
};
export const ROLE={WORKER:0,SOLDIER:1,HAULER:2,CAMERA:3};
export const TASK={IDLE:0,PICK_BUILD:1,BUILD:2,GATHER:3,DEPOSIT:4,MOVE:5,FIGHT:6,HAUL_PICK:7,HAUL_DROP:8};
export const FACTIONS=['Pioneer Union','Cinder Directorate'];
export const GROUND_TYPES=['Peaty ground','Damp loam','Iron-rich soil','Rocky upland'];
export const PLANT_TYPES=['Fern thicket','Blueleaf vines','Flowering scrub','Golden reedgrass'];
export const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
export function hash(x,y,s=1){let n=Math.imul(x^s,374761393)^Math.imul(y+17,668265263);n=Math.imul(n^(n>>>13),1274126177);return((n^(n>>>16))>>>0)/4294967295;}
export function noise(x,y,size,seed){let ix=Math.floor(x/size),iy=Math.floor(y/size),u=x/size-ix,v=y/size-iy;u=u*u*(3-2*u);v=v*v*(3-2*v);let a=hash(ix,iy,seed),b=hash(ix+1,iy,seed),c=hash(ix,iy+1,seed),d=hash(ix+1,iy+1,seed);return(a+(b-a)*u)*(1-v)+(c+(d-c)*u)*v;}
