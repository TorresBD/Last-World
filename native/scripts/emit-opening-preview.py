"""Emit compact inspection copies of real renderer captures; originals stay unchanged."""
from pathlib import Path
from PIL import Image
import base64,io
root=Path(__file__).resolve().parents[2]
for stage in range(4):
    path=root/f'evidence/opening-hud/opening-{stage}-1366x768.png'
    if not path.is_file():
        print(f'OPENING_REVIEW_MISSING:{stage}')
        continue
    output=io.BytesIO()
    Image.open(path).convert('RGB').save(output,format='JPEG',quality=82)
    print(f'OPENING_REVIEW_IMAGE:{stage}:'+base64.b64encode(output.getvalue()).decode(),flush=True)
