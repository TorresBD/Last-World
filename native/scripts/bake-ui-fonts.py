"""Bake licensed UI font glyph coverage/metrics, never physical game artwork."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
from fontTools.ttLib import TTFont
import hashlib,json,math
root=Path(__file__).resolve().parents[1]/'assets/ui'
characters=''.join(chr(i) for i in range(32,127))+'×°·–—…↑↓←→✓✕▶'
for name,file,size in [('body','IBMPlexSans-Regular.ttf',64),('numbers','IBMPlexSans-Medium.ttf',64),('heading','Rajdhani-SemiBold.ttf',80)]:
    source=root/file
    font=ImageFont.truetype(str(source),size)
    cmap=TTFont(source).getBestCmap()
    ascent,descent=font.getmetrics()
    glyphs={};images=[];x=y=row_height=0;width=1024
    digit_advance=max(font.getlength(str(i)) for i in range(10))
    for ch in characters:
        paint=ch if ord(ch) in cmap else '?'
        box=font.getbbox(paint,anchor='ls')
        left,top,right,bottom=box
        w=max(1,right-left);h=max(1,bottom-top)
        if x+w+4>width:x=0;y+=row_height+4;row_height=0
        glyph=Image.new('L',(w,h),0)
        ImageDraw.Draw(glyph).text((-left,-top),paint,font=font,fill=255,anchor='ls')
        images.append((glyph,x+2,y+2))
        advance=digit_advance if ch.isdigit() else font.getlength(paint)
        glyphs[str(ord(ch))]={'x':x+2,'y':y+2,'width':w,'height':h,'left':left,'top':top,'advance':round(advance,5),'fallback':paint!=ch}
        x+=w+4;row_height=max(row_height,h)
    height=1<<math.ceil(math.log2(max(1,y+row_height+4)))
    atlas=Image.new('L',(width,height),0)
    for glyph,x,y in images:atlas.paste(glyph,(x,y))
    atlas.save(root/(name+'-atlas.png'),optimize=True)
    metadata={'version':1,'font_file':file,'font_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'source_pixels':size,'ascent':ascent,'descent':descent,'line_height':ascent+descent,'width':width,'height':height,'tabular_digits':True,'glyphs':glyphs}
    (root/(name+'-atlas.json')).write_text(json.dumps(metadata,indent=2)+'\n')
    print(name,file,len(glyphs),'glyphs',width,height,(root/(name+'-atlas.png')).stat().st_size,'PNG bytes')
