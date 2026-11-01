"""Run native checks/builds and append stdout/stderr to the shared console history."""
from pathlib import Path
from datetime import datetime,timezone
import subprocess,sys,os
root=Path(__file__).resolve().parents[2]
args=sys.argv[1:]
if not args:raise SystemExit('Pass a command and its arguments')
with (root/'console.txt').open('a') as log:
    log.write('\n=== NATIVE '+datetime.now(timezone.utc).isoformat()+' ===\n'+repr(args)+'\n');log.flush()
    process=subprocess.Popen(args,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    for line in process.stdout:
        log.write(line);log.flush();print(line,end='',flush=True)
    code=process.wait();log.write('\nEXIT '+str(code)+'\n')
raise SystemExit(code)
