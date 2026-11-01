# Windows PowerShell 5.1: owned suspended launch and kill-on-supervisor-close job.
# No external compiler/runtime or administrative privileges are requested.
if (-not ('LastWorldCheckChild' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

public sealed class LastWorldCheckChild : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Security {
        public int length; public IntPtr descriptor;
        [MarshalAs(UnmanagedType.Bool)] public bool inherit;
    }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Startup {
        public int cb; public string reserved, desktop, title;
        public int x,y,xSize,ySize,xChars,yChars,fill,flags;
        public short show, reservedBytes; public IntPtr reserved2, input, output, error;
    }
    [StructLayout(LayoutKind.Sequential)] struct ProcessInfo {
        public IntPtr process, thread; public uint processId, threadId;
    }
    [StructLayout(LayoutKind.Sequential)] struct BasicLimits {
        public long processTime, jobTime; public uint flags;
        public UIntPtr minimumWorkingSet, maximumWorkingSet; public uint activeProcessLimit;
        public UIntPtr affinity; public uint priority, scheduling;
    }
    [StructLayout(LayoutKind.Sequential)] struct IoCounters {
        public ulong readOps,writeOps,otherOps,readBytes,writeBytes,otherBytes;
    }
    [StructLayout(LayoutKind.Sequential)] struct ExtendedLimits {
        public BasicLimits basic; public IoCounters io;
        public UIntPtr processMemory, jobMemory, peakProcessMemory, peakJobMemory;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateFile(string name,uint access,uint share,ref Security security,uint mode,uint flags,IntPtr template);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool CreateProcess(string application,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string directory,ref Startup startup,out ProcessInfo info);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateJobObject(IntPtr security,string name);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetInformationJobObject(IntPtr job,int kind,ref ExtendedLimits limits,uint size);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job,IntPtr process);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateProcess(IntPtr process,uint code);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateJobObject(IntPtr job,uint code);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
    [DllImport("kernel32.dll")] static extern uint GetErrorMode();
    [DllImport("kernel32.dll")] static extern uint SetErrorMode(uint mode);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    IntPtr job, thread;
    public Process Process { get; private set; }
    public bool Resumed { get; private set; }

    static void Check(bool value) { if (!value) throw new Win32Exception(Marshal.GetLastWin32Error()); }
    static string Quote(string argument) {
        StringBuilder result=new StringBuilder("\""); int slashes=0;
        foreach(char c in argument) {
            if(c=='\\') { slashes++; continue; }
            if(c=='\"') { result.Append('\\',slashes*2+1); result.Append(c); }
            else { result.Append('\\',slashes); result.Append(c); }
            slashes=0;
        }
        result.Append('\\',slashes*2); result.Append('"'); return result.ToString();
    }
    public static LastWorldCheckChild StartSuspended(string exe,string[] arguments,string directory,string stdout,string stderr,ulong memoryLimit,int benchmarkAttempt,string temporaryDirectory) {
        if(benchmarkAttempt<0||benchmarkAttempt>2) throw new ArgumentOutOfRangeException("benchmarkAttempt");
        IntPtr output=IntPtr.Zero,error=IntPtr.Zero,input=IntPtr.Zero,job=IntPtr.Zero,environment=IntPtr.Zero;
        ProcessInfo info=new ProcessInfo(); bool created=false;
        uint previousErrorMode=GetErrorMode();
        try {
            // Inherited by this diagnostic child: native faults become owned
            // exits/results instead of unattended Windows fault dialogs.
            SetErrorMode(previousErrorMode|0x8003);
            Security security=new Security { length=Marshal.SizeOf(typeof(Security)), inherit=true };
            output=CreateFile(stdout,0x40000000,3,ref security,2,0x80,IntPtr.Zero);
            error=CreateFile(stderr,0x40000000,3,ref security,2,0x80,IntPtr.Zero);
            input=CreateFile("NUL",0x80000000,3,ref security,3,0x80,IntPtr.Zero);
            if(output==new IntPtr(-1)||error==new IntPtr(-1)||input==new IntPtr(-1)) throw new Win32Exception(Marshal.GetLastWin32Error());
            job=CreateJobObject(IntPtr.Zero,null); Check(job!=IntPtr.Zero);
            // Hard process commit cap plus kill when the supervising process
            // loses its job handle. Polling supplies labelled guard-stop data.
            ExtendedLimits limits=new ExtendedLimits();
            limits.basic.flags=0x2000|0x100;
            limits.processMemory=new UIntPtr(memoryLimit);
            Check(SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(ExtendedLimits))));
            Startup startup=new Startup { cb=Marshal.SizeOf(typeof(Startup)),flags=0x100,input=input,output=output,error=error };
            StringBuilder command=new StringBuilder(Quote(exe));
            foreach(string argument in arguments) command.Append(" ").Append(Quote(argument));
            // Copy the parent's environment, then change only the child's
            // Unicode block. No process-global environment variable is set.
            SortedDictionary<string,string> variables=new SortedDictionary<string,string>(StringComparer.OrdinalIgnoreCase);
            foreach(DictionaryEntry variable in Environment.GetEnvironmentVariables()) variables[(string)variable.Key]=(string)variable.Value;
            variables.Remove("LAST_WORLD_BENCHMARK_ATTEMPT");
            if(benchmarkAttempt!=0) variables["LAST_WORLD_BENCHMARK_ATTEMPT"]=benchmarkAttempt.ToString(System.Globalization.CultureInfo.InvariantCulture);
            variables["TEMP"]=temporaryDirectory;
            variables["TMP"]=temporaryDirectory;
            StringBuilder block=new StringBuilder();
            foreach(KeyValuePair<string,string> variable in variables) block.Append(variable.Key).Append('=').Append(variable.Value).Append('\0');
            block.Append('\0');if(variables.Count==0) block.Append('\0');
            environment=Marshal.StringToHGlobalUni(block.ToString());
            Check(CreateProcess(exe,command,IntPtr.Zero,IntPtr.Zero,true,0x4|0x08000000|0x400,environment,directory,ref startup,out info));
            created=true; Check(AssignProcessToJobObject(job,info.process));
            Process process=Process.GetProcessById((int)info.processId);
            // Keep a kernel handle before resume so instant test exits still
            // have a reliable WaitForExit/ExitCode after the PID disappears.
            Check(process.Handle!=IntPtr.Zero);
            // Force creation-time retrieval while the exact process exists.
            process.StartTime.ToUniversalTime();
            LastWorldCheckChild child=new LastWorldCheckChild { job=job,thread=info.thread,Process=process };
            job=IntPtr.Zero; info.thread=IntPtr.Zero; return child;
        } catch {
            if(created&&info.process!=IntPtr.Zero) {
                TerminateProcess(info.process,0xe0000001);
                WaitForSingleObject(info.process,10000);
            }
            throw;
        } finally {
            SetErrorMode(previousErrorMode);
            if(environment!=IntPtr.Zero) Marshal.FreeHGlobal(environment);
            foreach(IntPtr handle in new[]{output,error,input,info.process,info.thread,job})
                if(handle!=IntPtr.Zero&&handle!=new IntPtr(-1)) CloseHandle(handle);
        }
    }
    public void Resume() {
        if(Resumed) throw new InvalidOperationException("Owned child already resumed");
        if(ResumeThread(thread)==0xffffffff) throw new Win32Exception(Marshal.GetLastWin32Error());
        Resumed=true; CloseHandle(thread); thread=IntPtr.Zero;
    }
    public void Stop() { if(job!=IntPtr.Zero) Check(TerminateJobObject(job,0xe0000002)); }
    public void Dispose() {
        if(thread!=IntPtr.Zero) { CloseHandle(thread); thread=IntPtr.Zero; }
        if(job!=IntPtr.Zero) { CloseHandle(job); job=IntPtr.Zero; }
        if(Process!=null) { Process.Dispose(); Process=null; }
    }
}
'@
}
