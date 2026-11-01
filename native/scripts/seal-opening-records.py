"""Seal source-local records only after the CI checks and first overlay audit pass."""
from pathlib import Path
import json,os,sys,zipfile
root=Path(__file__).resolve().parents[2]
if '--source-zip' in sys.argv:
    output=root/'dist/downloads/LastWorld-Source-B003-2026-10-31.zip'
    with zipfile.ZipFile(root/'dist/downloads/Last-World-Update-Windows.zip') as update,zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as source:
        names=[]
        for entry in update.infolist():
            if entry.filename.startswith('Last-World/Source/'):
                name=entry.filename.removeprefix('Last-World/Source/')
                source.writestr(name,update.read(entry));names.append(name)
        assert 'project/MASTER.md' in names and 'project/LEDGER.md' in names
        assert any(n.startswith('native/') for n in names)
    print(output)
    raise SystemExit(0)
stamp=json.loads((root/'native/target/x86_64-pc-windows-gnu/release/native-build-stamp.json').read_text())
report=json.loads((root/'evidence/opening-hud/report.json').read_text())
assert stamp['source_frozen_through_build'] and report['tick']==0 and report['paused'] is True
assert len(report['captures'])>=12 and all((root/n).is_file() for n in report['captures'])
counts={e['suite']:len(e['test_names']) for e in stamp['test_harnesses']}
assert all(counts.values())
sha=os.environ['GITHUB_SHA'];run=os.environ['GITHUB_RUN_ID'];exe=stamp['executable_sha256']
summary=(f"**This session (2026-10-31):** Recreated the seven interrupted opening/HUD/recon changes from mirror 9b89dc3. Windows CI compiled locked/pinned source and passed library/CLI regressions ({counts}); pixel-map data checks passed. The production renderer captured 12 opening frames at 1366x768, 1920x1080 and 2560x1440 with tick 0 preserved. Screenshots are generated evidence, awaiting visual inspection. First overlay audit passed; this candidate is being resealed with these matching source-local records. No performance or Tor-PC visual qualification follows.\n"
f"**Source:** {sha}; CI run {run}; native inputs {stamp['native_source_sha256']}; exe {exe}.\n"
"**Next:** Inspect the captured images, finish any visual corrections, and review Tor's actual-PC play feedback. All larger scope/performance gates remain open.\n"
"**Look up:** L-0026 and B-003; #opening #ui #build.\n")
assert len(summary)<=1500
master=root/'project/MASTER.md';text=master.read_text(encoding='utf-8')
a=text.index('## 1. SESSION SUMMARY');b=text.index('## 2.',a)
heading=text[a:text.index('\n',a)]
text=text[:a]+heading+'\n\n'+summary+'\n---\n\n'+text[b:]
text=text.replace('**Revision:** M10-WIP','**Revision:** M10')
a=text.index('## 7. CURRENT STATE');b=text.index('## 8.',a)
current=("## 7. CURRENT STATE (rewrite each session; current truth only)\n\n"
"**B-003 Windows CI candidate:** Section 8 source compiles; named library/CLI and pixel-map data checks passed in the recorded CI run. A small paused landing, both notices, crew identity/tasks/loadout/real bounded history and role commands, four camera choices, hand-plotted icons and sparse optical terrain/object memory are present. Legacy tool/save identities and full-count constructors remain.\n"
"**Visual evidence:** 12 production-renderer captures were written at small/1080p/1440p sizes. They need human/assistant inspection; creating them alone is not visual approval. Tor-PC playtesting and performance remain UNMEASURED for this candidate.\n"
"**Delivery evidence:** First overlay/source identity and user-data preservation audit passed. The matching records are included in the resealed overlay and dated complete Source zip; upload is conditional on the final audit passing. Source-native inputs and executable identities are in the build stamp. Historical evidence folders absent from the mirror are listed explicitly in the package manifest.\n"
"**Remaining scope:** The resident fixture remains 4.8x3.2km. Production20km streaming, complete mandatory biology, remaining130 mechanisms and all other open 1.0/performance gates are unchanged.\n\n---\n\n")
text=text[:a]+current+text[b:]
assert len(text)<30000
master.write_text(text,encoding='utf-8')
ledger=root/'project/LEDGER.md'
entry=(f"\nL-0026 | 2026-10-31 | CI_CANDIDATE | Locked Windows compilation and library/CLI checks passed ({counts}); authored icon data checked; 12 production-renderer frames captured at 1366/1920/2560 widths with paused tick0. First overlay identity/preservation audit passed; matching records resealed before final audit/upload. Visual inspection and Tor-PC performance remain pending/unmeasured. CI run {run}; commit {sha}. | #opening #ui #build\n"
f"B-003 | 2026-10-31 | {sha} | {exe} | opening/HUD/recon candidate | CI library/CLI + icon-data + production capture + first overlay audit; final sealing audit required | N | #build\n"
"\nLast entry: L-0026. Last approval: A-038. Last build: B-003.\n")
with ledger.open('a',encoding='utf-8') as out:out.write(entry)
print(json.dumps({'counts':counts,'source':sha,'native':stamp['native_source_sha256'],'exe':exe,'capture_count':12}))
