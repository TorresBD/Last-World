import {writeFileSync} from 'node:fs';
import {deflateSync} from 'node:zlib';
import {growPixelForm} from '../../dist/pixel-forms.mjs';
import {TYPES} from '../../dist/config.mjs';
const width=1024,height=1024,pixels=new Uint8Array(width*height*4),rects={};let small=0,big=0;
function add(key,kind,seed,team,role,pose){const f=growPixelForm(kind,seed,team,role,pose),x=f.size===16?small++%64*16:big++%16*64,y=f.size===16?Math.floor((small-1)/64)*16:128+Math.floor((big-1)/16)*64;rects[key]=[x,y,f.size,f.size];for(let row=0;row<f.size;row++)pixels.set(f.data.subarray(row*f.size*4,(row+1)*f.size*4),((y+row)*width+x)*4);}
for(let team=0;team<2;team++)for(let role=0;role<4;role++)for(let variant=0;variant<8;variant++)for(let pose=0;pose<4;pose++)add(`unit:${team}:${role}:${variant}:${pose}`,'unit',variant+9173,team,role,pose);
for(let species=0;species<4;species++)for(let variant=0;variant<4;variant++)for(let pose=0;pose<4;pose++)add(`fauna:${species}:${variant}:${pose}`,'fauna',variant+9173,0,species,pose);
for(const kind of [...Object.keys(TYPES),'beacon'])for(let team=0;team<2;team++)for(let variant=0;variant<4;variant++)add(`${kind}:${team}:${variant}`,kind,variant+9173,team,0,0);
// Lossless RGBA PNG, no external encoder or art dependency.
function crc32(bytes){let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let n=0;n<8;n++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return(crc^0xffffffff)>>>0;}
function chunk(type,data){const name=Buffer.from(type),head=Buffer.alloc(4),tail=Buffer.alloc(4);head.writeUInt32BE(data.length);tail.writeUInt32BE(crc32(Buffer.concat([name,data])));return Buffer.concat([head,name,data,tail]);}
const header=Buffer.alloc(13);header.writeUInt32BE(width);header.writeUInt32BE(height,4);header[8]=8;header[9]=6;
const rows=Buffer.alloc(height*(width*4+1));for(let y=0;y<height;y++)rows.set(pixels.subarray(y*width*4,(y+1)*width*4),y*(width*4+1)+1);
writeFileSync('native/assets/pixel-atlas.png',Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(rows,{level:9})),chunk('IEND',Buffer.alloc(0))]));writeFileSync('native/assets/pixel-atlas.json',JSON.stringify({width,height,rects}));
