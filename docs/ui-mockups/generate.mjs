/** Static UI design references. No game rendering or generated world artwork. */
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const out=path.dirname(new URL(import.meta.url).pathname),root=path.resolve(out,'../..');
const atlas=JSON.parse(fs.readFileSync(path.join(root,'native/assets/pixel-atlas.json')));
const refs=path.join(out,'references');fs.mkdirSync(refs,{recursive:true});
for(const[name,source]of [['pixel-atlas.png','native/assets/pixel-atlas.png'],['world-start.png','evidence/pc-coupled-2026-10-30/returned/river-check-17376-1793326377221-start.png'],['Rajdhani-SemiBold.ttf','native/assets/ui/Rajdhani-SemiBold.ttf'],['IBMPlexSans-Regular.ttf','native/assets/ui/IBMPlexSans-Regular.ttf'],['Rajdhani-OFL.txt','native/assets/ui/Rajdhani-OFL.txt'],['IBMPlex-OFL.txt','native/assets/ui/IBMPlex-OFL.txt']])fs.copyFileSync(path.join(root,source),path.join(refs,name));
const atlasPath='references/pixel-atlas.png';
const sharp=require('sharp');
const atlasRaw=await sharp(path.join(root,'native/assets/pixel-atlas.png')).ensureAlpha().raw().toBuffer({resolveWithObject:true});
const tightRects={};
for(const[key,[x,y,w,h]]of Object.entries(atlas.rects)){
 let minX=w,minY=h,maxX=-1,maxY=-1;
 for(let py=0;py<h;py++)for(let px=0;px<w;px++)if(atlasRaw.data[((y+py)*atlasRaw.info.width+x+px)*4+3]>24){minX=Math.min(minX,px);minY=Math.min(minY,py);maxX=Math.max(maxX,px);maxY=Math.max(maxY,py);}
 tightRects[key]=maxX<0?[x,y,w,h]:[x+Math.max(0,minX-1),y+Math.max(0,minY-1),Math.min(w,maxX+2)-Math.max(0,minX-1),Math.min(h,maxY+2)-Math.max(0,minY-1)];
}
const worldPath='references/world-start.png';
const T={frame:'#171D1F',panel:'#20282B',raised:'#263034',edge:'#65716D',text:'#EEE9D8',muted:'#ADB8AD',amber:'#E9B35A',blue:'#67B7E1',bio:'#83C991',damage:'#E57D70'};
const scenes=['calm-hud','battle-alerts','build-grid','expanded-species','food-web','logistics'];
const sizes=[[1920,1080],[1366,768],[2560,1440]];
const reviews=[];
const esc=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');
function render(scene,W,H){
 const small=W<1600,S=W>2200?1.2:1,F=small?14:16*S,HH=small?21:25*S,M=small?12:24;
 const hits=[],panels=[];
 const tx=(x,y,s,size=F,color=T.text,cls='body')=>`<text x="${x}" y="${y}" fill="${color}" font-size="${size}" class="${cls}">${esc(s)}</text>`;
 const line=(x,y,x2,y2,c=T.edge,dash='')=>`<path d="M${x} ${y} L${x2} ${y2}" stroke="${c}" stroke-width="1" fill="none" ${dash?`stroke-dasharray="${dash}"`:''}/>`;
 function panel(x,y,w,h,title,accent=T.amber,details=false){
  panels.push({x,y,w,h,title,default:!details});
  let a=`<g><path d="M${x+8} ${y} H${x+w-8} L${x+w} ${y+8} V${y+h-8} L${x+w-8} ${y+h} H${x+8} L${x} ${y+h-8} V${y+8} Z" fill="url(#metal)" stroke="#080D0E" stroke-width="3"/>
   <path d="M${x+8} ${y+2} H${x+w-8} L${x+w-2} ${y+8}" fill="none" stroke="${T.edge}"/>
   <rect x="${x+3}" y="${y+3}" width="${w-6}" height="${h-6}" fill="#9FA399" opacity=".055" filter="url(#grain)"/>
   <path d="M${x+18} ${y+2} h48 M${x+27} ${y+5} h20 M${x+w-52} ${y+h-3} h30" stroke="#D7D5BC" opacity=".23" stroke-width="1"/>
   <path d="M${x+5} ${y+12} v15 m4 -19 v15 M${x+w-7} ${y+h-12} v-15" stroke="${accent}" stroke-width="2"/>
   <path d="M${x+10} ${y+9} h5 m-2.5 -2.5 v5 M${x+w-16} ${y+9} h5 m-2.5 -2.5 v5" stroke="#65716D" opacity=".6"/>
   ${title?tx(x+20,y+30*S,title,HH,accent,'heading'):''}`;
  if(title)a+=line(x+18,y+42*S,x+w-18,y+42*S,'#46504D');
  if(details){a+=`<path d="M${x+w-60} ${y+16} l7 7 -7 7 M${x+w-34} ${y+16} l12 12 m0 -12 l-12 12" stroke="${T.muted}" stroke-width="1.5" fill="none"/>`;hits.push({kind:'pin',x:x+w-92,y:y,w:44,h:44},{kind:'close',x:x+w-44,y,w:44,h:44});}
  return a+'</g>';
 }
 function icon(kind,x,y,size=48,team=0,variant=0,dim=false){
  if(kind.startsWith('plant:'))return `<svg x="${x}" y="${y}" width="${size}" height="${size}" viewBox="744 36 104 116"><image href="${worldPath}" width="1280" height="800" style="image-rendering:pixelated"/></svg>`;
  let key=(kind.startsWith('fauna:')||kind.startsWith('unit:'))?kind:`${kind}:${team}:${variant}`;
  const r=tightRects[key]||tightRects['depot:0:0'];
  return `<svg x="${x}" y="${y}" width="${size}" height="${size}" viewBox="${r.join(' ')}"><image href="${atlasPath}" width="1024" height="1024" style="image-rendering:pixelated" ${dim?'filter="url(#silhouette)"':''}/></svg>`;
 }
 function button(x,y,w,h,kind,label,key,active=false){
  hits.push({kind:label,x,y,w,h});
  const compact=h<60*S;
  return `<g><path d="M${x+4} ${y} h${w-8} l4 4 v${h-8} l-4 4 h-${w-8} l-4 -4 v-${h-8} Z" fill="${active?'#3B3930':T.raised}" stroke="${active?T.amber:'#4B5755'}"/>
   <path d="M${x+5} ${y+2} h${w-10}" stroke="#79817A" opacity=".45"/>
   ${icon(kind,compact?x+10*S:x+(w-42*S)/2,compact?y+(h-36*S)/2:y+2,compact?36*S:42*S)}${tx(compact?x+56*S:x+8,compact?y+h/2+F*.34:y+h-9,label,Math.max(14,F-2),active?T.amber:T.text)}${tx(x+w-17,y+17,key,Math.max(12,F-2),T.muted,'heading')}</g>`;
 }
 function bar(x,y,w,pct,c=T.bio){return `<rect x="${x}" y="${y}" width="${w}" height="5" fill="#090F10"/><path d="M${x} ${y+2.5} h${w*pct}" stroke="${c}" stroke-width="5"/>`;}
 function paragraph(x,y,width,text,color=T.muted){
  const max=Math.floor(width/(F*.54));let lines=[],l='';for(const word of text.split(' ')){if((l+' '+word).trim().length>max){lines.push(l);l=word;}else l=(l+' '+word).trim();}if(l)lines.push(l);return lines.map((t,i)=>tx(x,y+i*(F+7),t,F,color)).join('');
 }
 let svg=`<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="Last World ${scene} static UI mockup">
 <defs><linearGradient id="metal" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#2C3639"/><stop offset=".08" stop-color="#20282B"/><stop offset="1" stop-color="#171D1F"/></linearGradient>
 <filter id="grain"><feTurbulence baseFrequency=".62" numOctaves="2" seed="27" stitchTiles="stitch"/><feColorMatrix type="saturate" values="0"/></filter>
 <filter id="silhouette"><feColorMatrix values="0 0 0 0 .045 0 0 0 0 .065 0 0 0 0 .06 0 0 0 1 0"/></filter>
 <pattern id="hazard" width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(35)"><rect width="6" height="12" fill="${T.amber}"/></pattern>
 <style>@font-face{font-family:'Rajdhani';src:url('references/Rajdhani-SemiBold.ttf');font-weight:600}@font-face{font-family:'IBM Plex Sans';src:url('references/IBMPlexSans-Regular.ttf');font-weight:400}.body{font-family:'IBM Plex Sans';font-weight:400}.heading{font-family:'Rajdhani';font-weight:600}text{font-variant-numeric:tabular-nums}</style></defs>
 <image href="${worldPath}" x="0" y="0" width="${W}" height="${H}" preserveAspectRatio="xMidYMid slice" style="image-rendering:pixelated"/>
 `;
 // Compact top resources. Iconography depicts the actual supply structures.
 const topW=small?762:1090*S,topH=small?60:70*S;
 svg+=panel(M,M,topW,topH,'');
 const stocks=small?[['mine','1,280','+24'],['mill','720','+12'],['farm','468','+8'],['solar','94','−6'],['depot','1,840','steady']]:[['mine','1,280','+24 / min'],['mill','720','+12 / min'],['farm','468','+8 / min'],['solar','94','−6 / min'],['depot','1,840','stocked']];
 let resourceX=M+14;const resourceGap=(topW-28)/5;const resourceNames={mine:'Metal',mill:'Timber',farm:'Food',solar:'Power',depot:'Supply'};
 for(const [kind,amount,rate]of stocks){svg+=icon(kind,resourceX,M+8,44*S)+tx(resourceX+48*S,M+28*S,amount,small?20:25*S,T.text,'heading')+tx(resourceX+48*S,M+49*S,resourceNames[kind]+' '+rate,small?14:14*S,rate.startsWith('−')?T.damage:T.bio);resourceX+=resourceGap;}
 const statW=small?280:350*S;
 svg+=panel(W-M-statW,M,statW,topH,'');
 svg+=`<rect x="${W-M-statW+13}" y="${M+9}" width="7" height="${topH-18}" fill="url(#hazard)"/>`+tx(W-M-statW+32,M+25*S,'EXPEDITION 01',small?19:23*S,T.amber,'heading')+tx(W-M-statW+32,M+48*S,'WET SEASON · 16:42',Math.max(14,F-1),T.text);
 svg+=tx(W-M-91*S,M+31*S,'1×',21*S,T.amber,'heading')+tx(W-M-41*S,M+31*S,'Ⅱ',20*S,T.muted,'heading');
 hits.push({kind:'speed cycle 1x 2x 4x',x:W-M-102*S,y:M+8,w:44*S,h:44*S},{kind:'pause',x:W-M-52*S,y:M+8,w:44*S,h:44*S});
 // Minimap and one-key layers.
 const mapW=small?188:244*S,mapH=small?178:226*S,mapY=H-M-mapH-38*S;
 svg+=panel(M,mapY,mapW,mapH+38*S,'SECTOR',T.amber);
 svg+=`<image href="${worldPath}" x="${M+10}" y="${mapY+47*S}" width="${mapW-20}" height="${mapH-18*S}" preserveAspectRatio="xMidYMid slice" style="image-rendering:pixelated" opacity=".72"/>
 <path d="M${M+mapW*.25} ${mapY+mapH*.42} h${mapW*.52} v${mapH*.41} h-${mapW*.52} Z" stroke="${T.text}" stroke-width="1" fill="none"/>
 <path d="M${M+18} ${mapY+mapH-8} l6 -10 6 10 Z" fill="${T.amber}"/>
 <path d="M${M+mapW-25} ${mapY+65} l8 0 0 8 -8 0 Z" fill="${T.blue}"/>`;
 svg+=tx(M+15,mapY+mapH+24*S,'M  Map',Math.max(14,F-1),T.muted)+tx(M+mapW-83*S,mapY+mapH+24*S,'+   -',18*S,T.text);
 hits.push({kind:'minimap',x:M+10,y:mapY+45*S,w:mapW-20,h:mapH-12*S},{kind:'minimap zoom in',x:M+mapW-96*S,y:mapY+mapH-6*S,w:44*S,h:44*S},{kind:'minimap zoom out',x:M+mapW-48*S,y:mapY+mapH-6*S,w:44*S,h:44*S});
 const layerY=mapY-52*S;svg+=panel(M,layerY,mapW,44*S,'');
 const layerKinds=['radar','camera','road','mine'];for(let i=0;i<4;i++){svg+=icon(layerKinds[i],M+8+i*(mapW-16)/4,layerY+2,38*S);hits.push({kind:`layer:${layerKinds[i]}`,x:M+5+i*(mapW-10)/4,y:layerY,w:(mapW-10)/4,h:44*S});}
 // Selection is a compact card, not a developer inspector.
 const selX=M+mapW+20*S,selW=small?348:460*S,selH=small?142:154*S,selY=H-M-selH;
 svg+=panel(selX,selY,selW,selH,'ENGINEER SQUAD',T.amber);
 svg+=icon('unit:0:0:0:0',selX+18,selY+51*S,60*S)+tx(selX+91*S,selY+64*S,'8 selected',F,T.text)+tx(selX+91*S,selY+88*S,'Building supply road',F,T.muted)+bar(selX+91*S,selY+103*S,selW-115*S,.72,T.amber)+tx(selX+91*S,selY+129*S,'Queue  1 road  ›  1 camera',Math.max(14,F-1),T.muted);
 // Build/select commands always remain discoverable at the right edge.
 const cmdW=small?304:398*S,cmdH=small?180:192*S,cmdX=W-M-cmdW,cmdY=H-M-cmdH;
 svg+=panel(cmdX,cmdY,cmdW,cmdH,'COMMAND',T.amber);
 const bw=(cmdW-32*S)/3,bh=small?62:66*S;
 const commands=[['road','Road','R'],['camera','Camera','C'],['depot','Build','B'],['trench','Trench','T'],['wall','Defend','D'],['fauna:0:0:0','Ecology','E']];
 commands.forEach((b,i)=>{svg+=button(cmdX+12*S+(i%3)*bw,cmdY+49*S+Math.floor(i/3)*(bh+6*S),bw-6*S,bh,b[0],b[1],b[2],scene==='build-grid'&&i===2);});
 // Compact ecological glance control: no exact unobserved species counts.
 const ecologyX=selX,ecologyY=selY-53*S;
 if(!small){svg+=panel(ecologyX,ecologyY,Math.min(selW,370*S),44*S,'');svg+=icon('fauna:0:0:0',ecologyX+9,ecologyY+3,36*S)+tx(ecologyX+57*S,ecologyY+28*S,'ECOLOGY   Stable  ↗',F,T.bio);hits.push({kind:'ecology',x:ecologyX,y:ecologyY,w:Math.min(selW,370*S),h:44*S});}
 // Contextual hint is short, fades, and can be disabled.
 if(scene==='calm-hud')svg+=`<rect x="${W/2-192*S}" y="${M+topH+12*S}" width="${384*S}" height="${34*S}" rx="3" fill="#111A18" opacity=".72"/>`+tx(W/2-178*S,M+topH+34*S,'Drag select · Right click order · Shift queue',Math.max(14,F-1),T.text);
 if(scene==='battle-alerts'){
  const ax=W-M-(small?310:374*S),ay=M+topH+18*S,aw=small?310:374*S;
  const alerts=[['CAMERA LOST','East approach · 2 merged reports',T.damage,'camera'],['SUPPLY BLOCKED','Road crossing flooded · view route',T.amber,'road'],['HABITAT AT RISK','Known nesting bank under fire',T.bio,'fauna:1:0:0']];
  alerts.forEach((a,i)=>{const y=ay+i*96*S;svg+=panel(ax,y,aw,84*S,'',a[2]);svg+=icon(a[3],ax+12,y+16*S,44*S)+tx(ax+70*S,y+29*S,a[0],20*S,a[2],'heading')+tx(ax+70*S,y+53*S,a[1],Math.max(14,F-2),T.text)+tx(ax+aw-20,y+31*S,'›',24*S,a[2]);hits.push({kind:'camera jump alert',x:ax,y,w:aw,h:84*S});});
 }
 if(scene==='build-grid'){
  const pw=small?480:560*S,px=W-M-pw,py=M+topH+22*S,ph=small?425:610*S;
  svg+=panel(px,py,pw,ph,'CONSTRUCTION',T.amber,true);
  svg+=tx(px+22*S,py+69*S,'Infrastructure   Defence   Biology',F,T.text)+line(px+22*S,py+80*S,px+146*S,py+80*S,T.amber);
  const builds=[['depot','Supply depot','Q'],['mine','Mine','W'],['mill','Processor','E'],['solar','Power','A'],['camera','Camera pole','S'],['radar','Radar','D'],['road','Road','Z'],['trench','Trench','X'],['wall','Wall','C']];
  const cw=(pw-44*S)/3,ch=small?82:100*S;
  builds.forEach((b,i)=>svg+=button(px+17*S+(i%3)*cw,py+94*S+Math.floor(i/3)*(ch+8*S),cw-8*S,ch,b[0],b[1],b[2],i===4));
  if(small)svg+=tx(px+22,py+ph-24,'Camera pole · 120 metal · 20 power',F,T.amber);
  if(!small){svg+=icon('camera',px+22*S,py+449*S,60*S)+tx(px+103*S,py+473*S,'CAMERA POLE',23*S,T.amber,'heading')+paragraph(px+103*S,py+499*S,pw-136*S,'Monitor an approach. Sight is blocked by solid terrain and structures.');svg+=tx(px+22*S,py+566*S,'120 metal   20 power   •   Shift to queue',F,T.text);}
 }
 if(scene==='expanded-species'){
  const px=small?225:320*S,py=small?96:118*S,pw=small?650:780*S,ph=small?458:652*S;
  svg+=panel(px,py,pw,ph,'ECOLOGY  /  COPPER BURROWER',T.bio,true);
  svg+=icon('fauna:1:0:0',px+22*S,py+62*S,120*S)+tx(px+171*S,py+88*S,'COPPER BURROWER',27*S,T.text,'heading')+tx(px+171*S,py+118*S,'Observed · declining in monitored banks',F,T.damage)+tx(px+171*S,py+149*S,'Habitat: moist soil / covered ground',F,T.muted)+bar(px+171*S,py+169*S,pw-203*S,.64,T.bio);
  const base=py+(small?202:223)*S;
  svg+=tx(px+24*S,base,'KNOWN TRAITS',20*S,T.bio,'heading')+tx(px+24*S,base+31*S,'Burrowing   •   Seasonal dormancy',F,T.text)+tx(px+24*S,base+58*S,'Den dependence   •   Dehydration tolerance',F,T.text);
  svg+=tx(px+24*S,base+104*S,'RESEARCH',20*S,T.amber,'heading')+tx(px+24*S,base+134*S,'Reproduction: partially understood',F,T.text)+tx(px+24*S,base+161*S,'Unobserved population: unknown',F,T.muted);
  const rx=px+pw*.57;svg+=tx(rx,base,'CONSERVATION',20*S,T.bio,'heading')+paragraph(rx,base+31*S,pw*.36,'Protect known entrances and keep connected moist habitat.')+tx(rx,base+109*S,'Captive stock  2 cared for',F,T.text)+tx(rx,base+139*S,'Local threat  Reservoir rise',F,T.damage);
  if(!small){svg+=line(px+24*S,py+477*S,px+pw-24*S,py+477*S)+tx(px+24*S,py+512*S,'RELATIONSHIPS',20*S,T.bio,'heading')+icon('fauna:0:0:0',px+25*S,py+530*S,42*S)+tx(px+82*S,py+557*S,'Known forage and predators  ›',F,T.text);}
  svg+=button(px+pw-226*S,py+ph-69*S,202*S,50*S,'restore','Protect habitat','P');
 }
 if(scene==='food-web'){
  const px=small?225:320*S,py=small?96:118*S,pw=small?920:1180*S,ph=small?458:650*S;
  svg+=panel(px,py,pw,ph,'ECOLOGY  /  KNOWN FOOD WEB',T.bio,true)+tx(px+24*S,py+70*S,'Known relationships · food → consumer · drag / scroll',F,T.muted);
  const gy=py+ph*.45,nodes=[['plant:canopy','Jade crown',.18,.42],['fauna:1:0:0','Copper burrower',.48,.26],['fauna:2:0:0','Indigo stalker',.79,.43],['fauna:3:0:0','Amber pollinator',.48,.72]];
  const pos=nodes.map(n=>[px+pw*n[2],py+ph*n[3]]);
  for(const[a,b,known]of [[0,1,true],[1,2,true],[3,2,true],[0,3,true]]){const A=pos[a],B=pos[b],dx=B[0]-A[0],dy=B[1]-A[1],len=Math.hypot(dx,dy),ux=dx/len,uy=dy/len;
   const ax=A[0]+Math.sign(dx)*50*S,ay=A[1],bx=B[0]-Math.sign(dx)*50*S,by=B[1],c=(ax+bx)/2;
   svg+=`<path d="M${ax} ${ay} C${c} ${ay},${c} ${by},${bx} ${by}" stroke="${known?T.bio:T.muted}" stroke-width="2" fill="none"/><path d="M${bx-ux*9*S-uy*4*S} ${by-uy*9*S+ux*4*S} L${bx} ${by} L${bx-ux*9*S+uy*4*S} ${by-uy*9*S-ux*4*S}" stroke="${T.bio}" stroke-width="2" fill="none"/>`;}
  nodes.forEach((n,i)=>{const[x,y]=pos[i];svg+=`<path d="M${x-46*S} ${y-46*S} h${92*S} v${92*S} h-${92*S}Z" fill="#162020" stroke="${i===1?T.amber:'#5A7965'}" stroke-width="1"/>`+icon(n[0],x-38*S,y-38*S,76*S)+tx(x-68*S,y+69*S,n[1],F,T.text);});
  svg+=tx(px+24*S,py+ph-27*S,'Select a species for known diet, threats and research.',F,T.muted)+tx(px+pw-104*S,py+ph-27*S,'−   100%   +',F,T.text);
 }
 if(scene==='logistics'){
  const px=small?225:320*S,py=small?96:118*S,pw=small?920:1180*S,ph=small?458:650*S;
  svg+=panel(px,py,pw,ph,'LOGISTICS  /  EASTERN SUPPLY',T.amber,true)+tx(px+24*S,py+70*S,'Stock and throughput · select a depot or a known route',F,T.muted);
  const rowY=py+126*S,col=pw/3;
  [['mine','Ore mine','240 / min',T.bio],['depot','Forward depot','1,840 stocked',T.amber],['barracks','East defence','Needs 80 / min',T.damage]].forEach((a,i)=>{const x=px+24*S+i*col;svg+=icon(a[0],x,rowY,80*S)+tx(x+95*S,rowY+27*S,a[1],22*S,a[3],'heading')+tx(x+95*S,rowY+57*S,a[2],F,T.text);if(i<2)svg+=`<path d="M${x+col-72*S} ${rowY+37*S} h${45*S} l-7 -6 m7 6 -7 6" stroke="${i===1?T.damage:T.bio}" stroke-width="2" fill="none"/>`;});
  const by=py+260*S;svg+=panel(px+22*S,by,pw-44*S,small?94:120*S,'',T.damage,true)+icon('road',px+36*S,by+18*S,60*S)+tx(px+111*S,by+33*S,'BOTTLENECK  /  FLOODED CROSSING',22*S,T.damage,'heading')+paragraph(px+111*S,by+64*S,pw-160*S,'East road is submerged. Haulers are using a longer safe route. Bridge or relocate this supply link.');
  if(!small){svg+=tx(px+24*S,py+432*S,'DELIVERY TREND',20*S,T.muted,'heading');for(let i=0;i<12;i++){const x=px+25*S+i*(pw-70*S)/12,h=(i<7?46+i*3:84-i*5)*S;svg+=`<rect x="${x}" y="${py+546*S-h}" width="${(pw-90*S)/16}" height="${h}" fill="${i<7?'#708C6D':T.damage}" opacity=".72"/>`;}svg+=tx(px+24*S,py+575*S,'4 min ago',14*S,T.muted)+tx(px+pw-90*S,py+575*S,'Now',14*S,T.muted);}
  svg+=button(px+pw-241*S,py+ph-69*S,217*S,50*S,'road','View safe route','V');
 }
 const proofLeft=selX+selW+14*S,proofRight=cmdX-14*S,proofW=Math.min(492*S,proofRight-proofLeft),proofX=proofLeft+(proofRight-proofLeft-proofW)/2;
 svg+=`<rect x="${proofX}" y="${H-29*S}" width="${proofW}" height="${24*S}" rx="2" fill="#0D1615" opacity=".86"/>`+tx(proofX+9*S,H-12*S,'UI MOCKUP · ILLUSTRATIVE DATA · EXISTING CAPTURE',Math.max(12,13*S),T.muted);
 svg+='</svg>';
 const hitErrors=hits.filter(r=>r.w<44||r.h<44);
 const boundsErrors=panels.filter(r=>r.x<0||r.y<0||r.x+r.w>W+.5||r.y+r.h>H+.5);
 const coverage=panels.filter(p=>p.default).reduce((a,p)=>a+p.w*p.h,0)/(W*H);
 reviews.push({scene,width:W,height:H,body_font_px:F,min_target_px:44,default_plate_coverage_fraction:coverage,hit_target_errors:hitErrors,bounds_errors:boundsErrors,world_source:worldPath,atlas_source:atlasPath,evidence:'static_UI_mockup_illustrative_data_not_native_screenshot'});
 return svg;
}
for(const scene of scenes)for(const[W,H]of sizes)fs.writeFileSync(path.join(out,`${scene}-${W}.svg`),render(scene,W,H));
fs.writeFileSync(path.join(out,'layout-review.json'),JSON.stringify(reviews,null,2)+'\n');
const rgb=s=>[1,3,5].map(i=>parseInt(s.slice(i,i+2),16)/255);const lum=s=>rgb(s).map(c=>c<=.04045?c/12.92:((c+.055)/1.055)**2.4).reduce((a,c,i)=>a+c*[.2126,.7152,.0722][i],0);
const contrast=Object.fromEntries(['text','muted','amber','blue','bio','damage'].map(k=>[k,(Math.max(lum(T[k]),lum(T.panel))+.05)/(Math.min(lum(T[k]),lum(T.panel))+.05)]));
fs.writeFileSync(path.join(out,'source-manifest.json'),JSON.stringify({kind:'static_UI_mockups_not_native_evidence',world_original:'evidence/pc-coupled-2026-10-30/returned/river-check-17376-1793326377221-start.png',atlas_original:'native/assets/pixel-atlas.png',physical_source_pixels_edited:false,reference_files:fs.readdirSync(refs).map(name=>({name,sha256:createHash('sha256').update(fs.readFileSync(path.join(refs,name))).digest('hex')})),font_families:['Rajdhani SemiBold','IBM Plex Sans Regular'],contrast_on_panel:contrast,render_engine:'Sharp/libvips/librsvg with bundled fonts through private fontconfig',chromium_available:false,inspection:'Source rectangle/hitbox audit plus manual PNG inspection; actual native runtime not established'},null,2)+'\n');

const cards=scenes.map((scene,i)=>`<button class="scene ${i===0?'selected':''}" data-scene="${scene}">${['Calm HUD','Battle alerts','Build grid','Expanded species','Food web','Logistics'][i]}</button>`).join('');
fs.writeFileSync(path.join(out,'index.html'),`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Last World UI — static design review</title><style>@font-face{font-family:Rajdhani;src:url('references/Rajdhani-SemiBold.ttf');font-weight:600}@font-face{font-family:'IBM Plex Sans';src:url('references/IBMPlexSans-Regular.ttf')}*{box-sizing:border-box}body{margin:0;background:#101719;color:#EEE9D8;font-family:'IBM Plex Sans',sans-serif}header{padding:20px 28px;display:flex;gap:24px;align-items:center;flex-wrap:wrap;border-bottom:1px solid #46504d}h1{margin:0;font-family:Rajdhani;font-size:30px;color:#E9B35A}p{font-size:14px;color:#ADB8AD;margin:8px 0}nav{display:flex;flex-wrap:wrap;gap:8px;padding:16px 28px}button{min-height:44px;background:#263034;border:1px solid #65716D;color:#EEE9D8;padding:9px 16px;font-family:inherit;font-size:14px;cursor:pointer}.selected{border-color:#E9B35A;color:#E9B35A}main{padding:0 28px 28px}#preview{display:block;width:100%;height:auto;max-height:calc(100vh - 210px);object-fit:contain;object-position:left top}a{color:#83C991}footer{padding:0 28px 28px;color:#ADB8AD;font-size:14px}</style><header><div><h1>LAST WORLD / UI FIELD TERMINAL</h1><p>STATIC DESIGN MOCKUPS · ILLUSTRATIVE DATA · EXISTING WORLD CAPTURE</p></div><div id="sizes"><button data-size="1366">1366 × 768</button> <button data-size="1920" class="selected">1920 × 1080</button> <button data-size="2560">2560 × 1440</button></div></header><nav>${cards}</nav><main><img id="preview" src="calm-hud-1920.png" alt="Static UI design reference"><p><a id="source" href="calm-hud-1920.svg">Open full-size SVG</a> · <a id="png" href="calm-hud-1920.png">Rendered PNG</a> · <a href="../UI_STYLE_DIRECTION.md">Style direction</a></p></main><footer>The game backdrop and portraits are existing assets. These layouts are design references, not proof of native UI behavior, gameplay or performance. Panels illustrate knowledge-filtered information; exact unseen population and enemy details remain unknown.</footer><script>let scene='calm-hud',size=1920;function show(){document.querySelector('#preview').src=scene+'-'+size+'.png';document.querySelector('#source').href=scene+'-'+size+'.svg';document.querySelector('#png').href=scene+'-'+size+'.png'}document.querySelectorAll('[data-scene]').forEach(b=>b.onclick=()=>{document.querySelectorAll('[data-scene]').forEach(x=>x.classList.toggle('selected',x===b));scene=b.dataset.scene;show()});document.querySelectorAll('[data-size]').forEach(b=>b.onclick=()=>{document.querySelectorAll('[data-size]').forEach(x=>x.classList.toggle('selected',x===b));size=+b.dataset.size;show()})</script></html>`);
if(process.argv.includes('--render')){
 const fontconf=path.join('/tmp',`last-world-ui-fonts-${process.pid}.conf`);
 fs.writeFileSync(fontconf,`<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><include ignore_missing="yes">/etc/fonts/fonts.conf</include><dir>${root}/native/assets/ui</dir><cachedir>/tmp/last-world-ui-font-cache</cachedir></fontconfig>`);
 process.env.FONTCONFIG_FILE=fontconf;
 for(const scene of scenes)for(const[W]of sizes){let source=fs.readFileSync(path.join(out,`${scene}-${W}.svg`),'utf8');for(const src of [atlasPath,worldPath])source=source.replaceAll(src,'data:image/png;base64,'+fs.readFileSync(path.resolve(out,src)).toString('base64'));await sharp(Buffer.from(source)).png().toFile(path.join(out,`${scene}-${W}.png`));}
 fs.unlinkSync(fontconf);
}
console.log(JSON.stringify({scenes:scenes.length,variants:reviews.length,layout_errors:reviews.flatMap(r=>r.bounds_errors).length,hit_errors:reviews.flatMap(r=>r.hit_target_errors).length,coverage:reviews.filter(r=>r.scene==='calm-hud').map(r=>({width:r.width,fraction:r.default_plate_coverage_fraction}))}));
