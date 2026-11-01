import {hash,TYPES,clamp} from './config.mjs';

// Appearance RNG is separate from growth RNG. Continuous natural HSV ranges.
export function proceduralFaunaColour(seed,species=0){
 const mix=n=>{n=(n^(n>>>16))>>>0;n=Math.imul(n,0x7feb352d)>>>0;n=(n^(n>>>15))>>>0;n=Math.imul(n,0x846ca68b)>>>0;return(n^(n>>>16))>>>0;};
 const key=mix((seed^Math.imul(species,0x9e3779b9)^Math.imul(37,0x85ebca6b))>>>0);
 const unit=salt=>(mix((key^Math.imul(salt,0x9e3779b9))>>>0)&65535)/65535;
 const choice=unit(1),r=choice<.26?[65,130,.25,.62,.30,.58]:choice<.64?[18,42,.20,.70,.22,.65]:choice<.80?[0,22,.30,.76,.28,.62]:choice<.93?[36,62,.38,.75,.48,.76]:[25,55,.03,.18,.12,.72];
 const range=i=>r[i]+(r[i+1]-r[i])*unit(i+3),h=range(0)/60,s=range(2),v=range(4),c=v*s,x=c*(1-Math.abs(h%2-1)),m=v-c;
 return(h<1?[c,x,0]:h<2?[x,c,0]:[0,c,x]).map(n=>(n+m)*255);
}

// All physical artwork grows as individual material cells. These are seeded
// walks/accretion, never a circle/rectangle/polygon filled to stand in for an object.
export function growPixelForm(kind,seed=1,team=0,role=0,pose=0){
 const small=kind==='unit'||kind==='fauna',size=small?16:64,cells=new Uint8Array(size*size),cx=size>>1,cy=size>>1;
 let randomState=(seed^0x6d2b79f5)>>>0;const rand=()=>{randomState=(Math.imul(randomState,1664525)+1013904223)>>>0;return randomState/4294967296;};
 const put=(x,y,v)=>{if(x>1&&y>1&&x<size-2&&y<size-2)cells[y*size+x]=v;};
 function walk(x,y,length,biasX=0,biasY=0,material=1){for(let n=0;n<length;n++){put(x,y,material);const r=rand();if(r<.25)x++;else if(r<.5)x--;else if(r<.75)y++;else y--;if(n%3===0){x+=Math.sign(biasX);y+=Math.sign(biasY);}x=clamp(x,2,size-3);y=clamp(y,2,size-3);}}
 if(small){
  for(let n=0;n<3;n++)walk(cx,cy,kind==='fauna'?11:9,0,-1,1);
  walk(cx,cy-2,5,0,-1,2);
  const stride=pose%2?1:-1;
  walk(cx-1,cy+1,4,-stride,1,3);walk(cx+1,cy+1,4,stride,1,3);
  walk(cx,cy-1,kind==='fauna'?6:3,-1,0,1);walk(cx,cy-1,kind==='fauna'?6:3,1,0,1);
  if(kind==='unit'&&role===1)walk(cx+2,cy-2,5,1,-1,4);
  if(kind==='unit'&&(role===2||role===3))walk(cx-2,cy,6,-1,0,role===3?5:4);
 }else{
  const long=kind==='ship',spread=kind==='beacon'?8:kind==='mountedCamera'?5:kind==='camera'?7:kind==='radar'?12:long?24:18;
  const frontier=[[cx,cy]],target=kind==='beacon'?150:kind==='camera'||kind==='mountedCamera'?90:long?1300:650;
  put(cx,cy,1);
  for(let n=0;n<target*6&&frontier.length<target;n++){
   const [x,y]=frontier[Math.floor(rand()*frontier.length)],d=Math.floor(rand()*4),nx=x+[1,-1,0,0][d],ny=y+[0,0,1,-1][d];
   if(Math.abs(nx-cx)>spread||Math.abs(ny-cy)>(long?spread*.55:spread)||nx<2||ny<2||nx>=size-2||ny>=size-2||cells[ny*size+nx])continue;
   put(nx,ny,1);frontier.push([nx,ny]);
  }
  const arms=kind==='solar'?7:kind==='radar'||kind==='tower'?5:kind==='ship'?12:6;
  for(let j=0;j<arms;j++)walk(cx,cy,spread+Math.floor(rand()*spread),j%2?1:-1,j%3-1,2);
  for(let y=2;y<size-2;y++)for(let x=2;x<size-2;x++)if(cells[y*size+x]){
   const grain=hash(x,y,seed),rib=(x+Math.floor(hash(y,0,seed)*3))%5===0;
   if(kind==='solar')cells[y*size+x]=rib?2:5;
   else if(kind==='farm')cells[y*size+x]=grain>.38?6:2;
   else if(kind==='mill')cells[y*size+x]=rib?3:1;
   else if(kind==='mine')cells[y*size+x]=grain>.72?4:1;
   else if(kind==='ship')cells[y*size+x]=grain>.86?7:rib?2:1;
   else if(kind==='beacon')cells[y*size+x]=grain>.62?5:2;
   else if(grain>.88)cells[y*size+x]=5;
  }
 }
 // Role/seed still select existing forms; palette selection consumes no RNG.
 const data=new Uint8ClampedArray(size*size*4),cloth=team?[143,110,77]:[91,128,121],palettes=small?[[0,0,0],kind==='fauna'?proceduralFaunaColour(seed,role):cloth,[171,152,115],[55,64,49],[109,112,89],[166,185,121]]:[[0,0,0],[108,113,98],[147,151,127],[101,82,54],[69,74,66],team?[128,148,145]:[76,123,134],[78,129,64],[65,51,39]];
 for(let y=1;y<size-1;y++)for(let x=1;x<size-1;x++){const i=y*size+x,m=cells[i];if(!m)continue;const edge=!cells[i-1]||!cells[i+1]||!cells[i-size]||!cells[i+size],grain=Math.trunc(hash(x+331,y+719,seed)*29)-14,shade=edge?-.18:.04;const palette=palettes[m]||palettes[1];for(let c=0;c<3;c++)data[i*4+c]=clamp(palette[c]*(1+shade)+grain,0,255);data[i*4+3]=255;}
 return {size,data,cells};
}

export class PixelForms{
 constructor(){this.pages=new Map();}
 get(kind,seed,team=0,role=0,pose=0){
  const small=kind==='unit'||kind==='fauna',tile=small?16:64,variant=(seed>>>0)%(small?8:4),key=[kind,variant,team,role,pose].join(':');let page=this.pages.get(tile);
  if(!page){const canvas=document.createElement('canvas');canvas.width=canvas.height=small?512:1024;page={canvas,context:canvas.getContext('2d'),entries:new Map(),cursor:0,tile,across:canvas.width/tile};this.pages.set(tile,page);}
  let item=page.entries.get(key);if(!item){const slot=page.cursor++%(page.across*page.across),x=slot%page.across*tile,y=Math.floor(slot/page.across)*tile;for(const [old,value]of page.entries)if(value.slot===slot){page.entries.delete(old);break;}
   const form=growPixelForm(kind,variant+9173,team,role,pose);page.context.putImageData(new ImageData(form.data,tile,tile),x,y);item={page,x,y,tile,slot};page.entries.set(key,item);
  }return item;
 }
 draw(c,kind,seed,team,role,pose,x,y,width,height=width,alpha=1){const a=this.get(kind,seed,team,role,pose);c.globalAlpha=alpha;c.drawImage(a.page.canvas,a.x,a.y,a.tile,a.tile,Math.round(x-width/2),Math.round(y-height/2),width,height);c.globalAlpha=1;}
}
