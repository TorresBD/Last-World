"""Validate authored icon data only; never render or generate artwork."""
import json,itertools,math
from pathlib import Path
root=Path(__file__).resolve().parents[2]
data=json.loads((root/'assets/icons/manifest.json').read_text())
palette=data['palette']; icons=data['icons']
assert len(palette)<=12
for key,icon in icons.items():
    for name,size in [('small',16),('large',32)]:
        rows=icon[name]
        assert len(rows)==size and all(len(r)==size for r in rows),key
        assert set(''.join(rows))<=set(palette),key
    rows=icon['small']
    assert all(c=='.' for c in rows[0]+rows[-1]),key
    assert all(r[0]==r[-1]=='.' for r in rows),key
for (a,x),(b,y) in itertools.combinations(icons.items(),2):
    one=''.join(x['small']);two=''.join(y['small'])
    assert sum(p==q for p,q in zip(one,two))/256<=.90,(a,b,'too similar')
# Camera choices must be recognizably different silhouettes.
for a,b in itertools.combinations(['camera','camera_pole','camera_tower','camera_mounted'],2):
    x={i for i,c in enumerate(''.join(icons[a]['small'])) if c!='.'}
    y={i for i,c in enumerate(''.join(icons[b]['small'])) if c!='.'}
    assert len(x&y)/len(x|y)<.80,(a,b,'silhouette')
def luminance(rgb):
    def linear(v):
        v=v/255
        return v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4
    return sum(linear(v)*w for v,w in zip(rgb,[.2126,.7152,.0722]))
bg=luminance([38,48,52])
for key in ['s','m','h','t','w','l','g','a','b','r']:
    fg=luminance(palette[key][:3])
    assert (max(fg,bg)+.05)/(min(fg,bg)+.05)>=3,(key,'contrast')
print(f"PASS: {len(icons)} icons; both sizes; symbols; padding; uniqueness; camera silhouettes; contrast")
