# Optional read-only Windows hardware telemetry. No elevated access or vendor tools.
# PDH paths are English/localized by the API; every returned instance is PID-checked.
if (-not ('LastWorldHardwareTelemetry' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;

public sealed class LastWorldHardwareTelemetry : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct FileTime { public uint low,high; public ulong Value { get { return ((ulong)high<<32)|low; } } }
    [StructLayout(LayoutKind.Sequential)] sealed class MemoryStatus {
        public uint length=(uint)Marshal.SizeOf(typeof(MemoryStatus)),load;
        public ulong totalPhysical,availablePhysical,totalPageFile,availablePageFile,totalVirtual,availableVirtual,availableExtendedVirtual;
    }
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetSystemTimes(out FileTime idle,out FileTime kernel,out FileTime user);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GlobalMemoryStatusEx([In,Out] MemoryStatus status);
    [DllImport("pdh.dll",CharSet=CharSet.Unicode,ExactSpelling=true)] static extern uint PdhOpenQueryW(string source,UIntPtr user,out IntPtr query);
    [DllImport("pdh.dll",CharSet=CharSet.Unicode,ExactSpelling=true)] static extern uint PdhAddEnglishCounterW(IntPtr query,string path,UIntPtr user,out IntPtr counter);
    [DllImport("pdh.dll",CharSet=CharSet.Unicode,ExactSpelling=true)] static extern uint PdhAddCounterW(IntPtr query,string path,UIntPtr user,out IntPtr counter);
    [DllImport("pdh.dll",CharSet=CharSet.Unicode,ExactSpelling=true)] static extern uint PdhGetCounterInfoW(IntPtr counter,bool explain,ref uint bytes,IntPtr info);
    [DllImport("pdh.dll",CharSet=CharSet.Unicode,ExactSpelling=true)] static extern uint PdhGetFormattedCounterArrayW(IntPtr counter,uint format,ref uint bytes,out uint count,IntPtr items);
    [DllImport("pdh.dll")] static extern uint PdhRemoveCounter(IntPtr counter);
    [DllImport("pdh.dll")] static extern uint PdhCollectQueryData(IntPtr query);
    [DllImport("pdh.dll")] static extern uint PdhCloseQuery(IntPtr query);
    [StructLayout(LayoutKind.Sequential)] struct InfoPrefix {
        public uint length,type,version,status; public int scale,defaultScale;
        public UIntPtr user,queryUser; public IntPtr fullPath;
    }
    [StructLayout(LayoutKind.Sequential)] struct FormattedValue { public uint status; public double value; }
    [StructLayout(LayoutKind.Sequential)] struct FormattedItem { public IntPtr name; public FormattedValue value; }
    public sealed class CounterItem { public string instance,adapter_luid,physical_adapter,engine,kind,status; public double? value; }
    public sealed class CounterSet { public string status; public List<CounterItem> instances=new List<CounterItem>(); }
    public sealed class CpuData {
        public string process_status="warmup",system_status="warmup";
        public int reported_logical_processors=Environment.ProcessorCount;
        public double? process_cpu_time_seconds,process_percent_one_logical_cpu,process_percent_reported_logical_capacity,system_busy_percent;
        public string denominator_note="Environment.ProcessorCount visible to supervisor; GetSystemTimes is calling processor-group scoped above64 logical CPUs";
    }
    public sealed class MemoryData {
        public string status="unavailable"; public ulong? total_physical_bytes,available_physical_bytes,used_physical_bytes,effective_commit_limit_bytes,available_effective_commit_bytes;
        public string commit_note="GlobalMemoryStatusEx total/available page-file fields are effective commit limits (system or calling-process limit, whichever smaller), not disk page-file size or VRAM";
    }
    public sealed class GpuData {
        public string status="not_requested",collection_status,interop_layout_status; public double? collection_ms;
        public Dictionary<string,string> counter_setup_errors=new Dictionary<string,string>();
        public CounterSet engines=new CounterSet(),dedicated_process_memory=new CounterSet(),shared_process_memory=new CounterSet();
        public double? busiest_engine_percent,engine_3d_max_percent,engine_compute_max_percent,engine_copy_max_percent;
        public int valid_engine_count,engine_3d_count,engine_compute_count,engine_copy_count;
        public double? dedicated_process_bytes,shared_process_bytes;
        public GpuData() {engines.status=dedicated_process_memory.status=shared_process_memory.status="not_requested";}
        public string scope="Owned process WDDM counters; busiest engine is a max, never a sum/whole-adapter percent. Dedicated process allocation is not total physical VRAM/residency. Shared allocations and driver counter defects can affect values.";
    }
    public sealed class SampleData { public string ownership_status; public CpuData cpu=new CpuData(); public MemoryData memory=new MemoryData(); public GpuData gpu=new GpuData(); }
    const int MaxBufferBytes=1024*1024,MaxInstances=512;
    readonly int pid; readonly long startTicks; readonly bool gpuRequested;
    readonly Stopwatch elapsed=Stopwatch.StartNew();
    readonly Dictionary<string,IntPtr> counters=new Dictionary<string,IntPtr>();
    readonly Dictionary<string,string> counterErrors=new Dictionary<string,string>();
    IntPtr query; string gpuError,layoutStatus="not_requested"; bool haveCpu,haveSystem,gpuPrimed; double previousCpuWall,previousCpu; ulong previousIdle,previousKernel,previousUser;
    static string Code(uint code) { return "0x"+code.ToString("X8"); }
    static bool Valid(uint status) { return status==0||status==1; }
    static double? Max(double? left,double value) { return left.HasValue?Math.Max(left.Value,value):value; }
    public LastWorldHardwareTelemetry(int ownedPid,long ownedStartTicks,bool includeGpu) {
        pid=ownedPid;startTicks=ownedStartTicks;gpuRequested=includeGpu;
        if(!includeGpu) return;
        try {
            int valueSize=Marshal.SizeOf(typeof(FormattedValue)),itemSize=Marshal.SizeOf(typeof(FormattedItem));
            int pathOffset=(int)Marshal.OffsetOf(typeof(InfoPrefix),"fullPath");
            if(valueSize!=16||itemSize!=24||pathOffset!=(IntPtr.Size==8?40:32)) throw new InvalidOperationException("PDH interop layout probe failed");
            layoutStatus="passed_value16_item24_path_offset"+pathOffset;
            uint result=PdhOpenQueryW(null,UIntPtr.Zero,out query);
            if(result!=0) throw new InvalidOperationException("PdhOpenQuery "+Code(result));
            Add("engines",@"\GPU Engine(pid_"+pid+@"_*)\Utilization Percentage");
            Add("dedicated",@"\GPU Process Memory(pid_"+pid+@"_*)\Dedicated Usage");
            Add("shared",@"\GPU Process Memory(pid_"+pid+@"_*)\Shared Usage");
        } catch(Exception error) { gpuError=error.Message; if(query!=IntPtr.Zero) {PdhCloseQuery(query);query=IntPtr.Zero;} }
    }
    void Add(string key,string englishPath) {
        IntPtr translator=IntPtr.Zero,info=IntPtr.Zero;uint bytes=0;
        try {
            uint status=PdhAddEnglishCounterW(query,englishPath,UIntPtr.Zero,out translator);
            if(status!=0) throw new InvalidOperationException("PdhAddEnglishCounter "+Code(status));
            status=PdhGetCounterInfoW(translator,false,ref bytes,IntPtr.Zero);
            if(status!=0x800007D2||bytes<Marshal.SizeOf(typeof(InfoPrefix))||bytes>MaxBufferBytes) throw new InvalidOperationException("PdhGetCounterInfo size "+Code(status));
            info=Marshal.AllocHGlobal((int)bytes);
            status=PdhGetCounterInfoW(translator,false,ref bytes,info);
            if(status!=0) throw new InvalidOperationException("PdhGetCounterInfo "+Code(status));
            InfoPrefix prefix=(InfoPrefix)Marshal.PtrToStructure(info,typeof(InfoPrefix));
            string localized=Marshal.PtrToStringUni(prefix.fullPath);
            if(String.IsNullOrEmpty(localized)) throw new InvalidOperationException("PDH returned an empty localized path");
            // Remove translation-only registration before adding the actual
            // wildcard counter, so no duplicate-path handle can be removed later.
            PdhRemoveCounter(translator);translator=IntPtr.Zero;
            // PdhAddCounter explicitly supports a wildcard query plus formatted
            // array reads. PdhAddEnglishCounter is used only for localization.
            IntPtr counter;status=PdhAddCounterW(query,localized,UIntPtr.Zero,out counter);
            if(status!=0) throw new InvalidOperationException("PdhAddCounter localized wildcard "+Code(status));
            counters[key]=counter;
        } catch(Exception error) {counterErrors[key]=error.Message;}
        finally {if(info!=IntPtr.Zero) Marshal.FreeHGlobal(info);if(translator!=IntPtr.Zero) PdhRemoveCounter(translator);}
    }
    CounterSet Read(string key,bool warmup) {
        CounterSet result=new CounterSet();IntPtr counter;
        if(!counters.TryGetValue(key,out counter)) {result.status=counterErrors.ContainsKey(key)?"unavailable: "+counterErrors[key]:"unavailable";return result;}
        uint bytes=0,count=0;IntPtr buffer=IntPtr.Zero;
        uint format=key=="engines"?0x200u|0x8000u:0x200u|0x1000u|0x8000u; // DOUBLE|NOCAP100 or DOUBLE|NOSCALE|NOCAP100(bytes)
        try {
            uint status=PdhGetFormattedCounterArrayW(counter,format,ref bytes,out count,IntPtr.Zero);
            if(status!=0x800007D2) {result.status="no_valid_data: "+Code(status);return result;}
            if(bytes>MaxBufferBytes||count>MaxInstances||bytes==0) {result.status="bounded_buffer_or_instance_limit";return result;}
            buffer=Marshal.AllocHGlobal((int)bytes);
            status=PdhGetFormattedCounterArrayW(counter,format,ref bytes,out count,buffer);
            if(status!=0) {result.status="no_valid_data: "+Code(status);return result;}
            int stride=Marshal.SizeOf(typeof(FormattedItem));
            if(count>MaxInstances||(ulong)count*(ulong)stride>bytes) {result.status="bounded_array_layout_limit";return result;}
            for(int index=0;index<count;index++) {
                FormattedItem item=(FormattedItem)Marshal.PtrToStructure(IntPtr.Add(buffer,index*stride),typeof(FormattedItem));
                string name=Marshal.PtrToStringUni(item.name);
                Match owner=Regex.Match(name??"",@"^pid_(\d+)_",RegexOptions.CultureInvariant);
                int instancePid;if(!owner.Success||!Int32.TryParse(owner.Groups[1].Value,out instancePid)||instancePid!=pid) continue;
                CounterItem value=new CounterItem {instance=name,status=Code(item.value.status)};
                Match adapter=Regex.Match(name,@"_luid_(0x[0-9a-f]+_0x[0-9a-f]+)_phys_(\d+)",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant);
                if(adapter.Success) {value.adapter_luid=adapter.Groups[1].Value;value.physical_adapter=adapter.Groups[2].Value;}
                Match engine=Regex.Match(name,@"_eng_(\d+)_engtype_([^#]+)",RegexOptions.IgnoreCase|RegexOptions.CultureInvariant);
                if(engine.Success) {value.engine=engine.Groups[1].Value;value.kind=engine.Groups[2].Value;}
                if(Valid(item.value.status)&&!Double.IsNaN(item.value.value)&&!Double.IsInfinity(item.value.value)&&item.value.value>=0&&!(warmup&&key=="engines")) value.value=item.value.value;
                result.instances.Add(value);
            }
            result.status=warmup&&key=="engines"?"warmup":result.instances.Count==0?"no_owned_instances":"sampled_check_instance_status";
        } catch(Exception error) {result.status="unavailable: "+error.Message;}
        finally {if(buffer!=IntPtr.Zero) Marshal.FreeHGlobal(buffer);}
        return result;
    }
    public SampleData Sample(Process ownedProcess) {
        SampleData row=new SampleData();
        ownedProcess.Refresh();
        if(ownedProcess.HasExited||ownedProcess.Id!=pid||ownedProcess.StartTime.ToUniversalTime().Ticks!=startTicks) {row.ownership_status="owned_process_gone_or_mismatch";row.cpu.process_status=row.cpu.system_status=row.memory.status=row.gpu.status="not_sampled_ownership_guard";return row;}
        row.ownership_status="retained_process_handle_and_creation_ticks_match";
        double wall=elapsed.Elapsed.TotalSeconds;
        try {
            double cpu=ownedProcess.TotalProcessorTime.TotalSeconds;row.cpu.process_cpu_time_seconds=cpu;
            if(haveCpu&&wall>previousCpuWall&&cpu>=previousCpu) {
                row.cpu.process_percent_one_logical_cpu=(cpu-previousCpu)/(wall-previousCpuWall)*100.0;
                row.cpu.process_percent_reported_logical_capacity=row.cpu.process_percent_one_logical_cpu/row.cpu.reported_logical_processors;
                row.cpu.process_status="sampled";
            }
            previousCpu=cpu;previousCpuWall=wall;haveCpu=true;
        } catch(Exception error) {row.cpu.process_status="unavailable: "+error.Message;}
        FileTime idle,kernel,user;
        if(GetSystemTimes(out idle,out kernel,out user)) {
            if(haveSystem&&kernel.Value>=previousKernel&&user.Value>=previousUser&&idle.Value>=previousIdle) {
                ulong total=(kernel.Value-previousKernel)+(user.Value-previousUser),idleDelta=idle.Value-previousIdle;
                if(total>0&&idleDelta<=total) {row.cpu.system_busy_percent=(double)(total-idleDelta)/total*100.0;row.cpu.system_status="sampled";} else row.cpu.system_status="invalid_delta";
            }
            previousIdle=idle.Value;previousKernel=kernel.Value;previousUser=user.Value;haveSystem=true;
        } else row.cpu.system_status="unavailable_win32_"+Marshal.GetLastWin32Error();
        MemoryStatus memory=new MemoryStatus();
        if(GlobalMemoryStatusEx(memory)) {
            row.memory.status="sampled";row.memory.total_physical_bytes=memory.totalPhysical;row.memory.available_physical_bytes=memory.availablePhysical;
            if(memory.availablePhysical<=memory.totalPhysical) row.memory.used_physical_bytes=memory.totalPhysical-memory.availablePhysical;
            row.memory.effective_commit_limit_bytes=memory.totalPageFile;row.memory.available_effective_commit_bytes=memory.availablePageFile;
        } else row.memory.status="unavailable_win32_"+Marshal.GetLastWin32Error();
        if(gpuRequested) {
            row.gpu.interop_layout_status=layoutStatus;row.gpu.counter_setup_errors=new Dictionary<string,string>(counterErrors);
            row.gpu.status=gpuError==null?"sampling":"unavailable: "+gpuError;
            if(query!=IntPtr.Zero) {
                Stopwatch gpuWatch=Stopwatch.StartNew();
                uint collected=PdhCollectQueryData(query);row.gpu.collection_status=Code(collected);
                if(collected==0) {
                bool warmup=!gpuPrimed;
                row.gpu.engines=Read("engines",warmup);row.gpu.dedicated_process_memory=Read("dedicated",false);row.gpu.shared_process_memory=Read("shared",false);
                foreach(CounterItem item in row.gpu.engines.instances) if(item.value.HasValue) {
                    double value=item.value.Value;row.gpu.valid_engine_count++;row.gpu.busiest_engine_percent=Max(row.gpu.busiest_engine_percent,value);
                    string kind=(item.kind??"").ToLowerInvariant();
                    if(kind=="3d") {row.gpu.engine_3d_count++;row.gpu.engine_3d_max_percent=Max(row.gpu.engine_3d_max_percent,value);}
                    else if(kind.StartsWith("compute")) {row.gpu.engine_compute_count++;row.gpu.engine_compute_max_percent=Max(row.gpu.engine_compute_max_percent,value);}
                    else if(kind=="copy") {row.gpu.engine_copy_count++;row.gpu.engine_copy_max_percent=Max(row.gpu.engine_copy_max_percent,value);}
                }
                foreach(CounterItem item in row.gpu.dedicated_process_memory.instances) if(item.value.HasValue) row.gpu.dedicated_process_bytes=(row.gpu.dedicated_process_bytes??0)+item.value.Value;
                foreach(CounterItem item in row.gpu.shared_process_memory.instances) if(item.value.HasValue) row.gpu.shared_process_bytes=(row.gpu.shared_process_bytes??0)+item.value.Value;
                row.gpu.status=warmup?"warmup":row.gpu.valid_engine_count>0?"sampled":"no_valid_engine_data_check_counter_status";
                gpuPrimed=true;
                } else {
                    gpuPrimed=false;row.gpu.status="collection_unavailable";
                    row.gpu.engines.status=row.gpu.dedicated_process_memory.status=row.gpu.shared_process_memory.status="not_read_after_collection_failure: "+Code(collected);
                }
                gpuWatch.Stop();row.gpu.collection_ms=gpuWatch.Elapsed.TotalMilliseconds;
                // PDH is synchronous; a slow call cannot be interrupted safely.
                // Record that call and disable later GPU reads over this budget.
                if(gpuWatch.Elapsed.TotalMilliseconds>250) {gpuError="prior_gpu_sample_exceeded250ms_budget";Dispose();}
            } else row.gpu.engines.status=row.gpu.dedicated_process_memory.status=row.gpu.shared_process_memory.status=row.gpu.status;
        }
        return row;
    }
    public void Dispose() {if(query!=IntPtr.Zero) {PdhCloseQuery(query);query=IntPtr.Zero;}counters.Clear();}
}
'@
}

function Start-LastWorldHardwareTelemetry($Process,$Owner,[bool]$IncludeGpu,[string]$Path) {
    $watch=[System.Diagnostics.Stopwatch]::StartNew()
    $state=@{sampler=$null;writer=$null;collector_process=$null;collector_cpu_ms_total=$null;collector_cpu_ms_max=$null;last_collector_cpu_ms=$null;path=$Path;segments=(New-Object 'System.Collections.Generic.List[string]');segment_index=0;segment_bytes=0L;segment_limit_bytes=8MB;extrema=@{process_cpu_one_logical_cpu_max_percent=$null;process_cpu_logical_capacity_max_percent=$null;system_cpu_max_percent=$null;gpu_busiest_engine_max_percent=$null;gpu_3d_max_percent=$null;gpu_compute_max_percent=$null;gpu_copy_max_percent=$null;gpu_dedicated_process_max_bytes=$null;gpu_shared_process_max_bytes=$null;system_available_physical_min_bytes=$null};gpu_requested=$IncludeGpu;owner=$Owner;samples=0;valid_gpu_samples=0;valid_cpu_samples=0;valid_ram_samples=0;schema_probe='not_run';setup_ms=0.0;sample_and_log_ms_total=0.0;sample_and_log_ms_max=0.0;error=$null;status='sampling';last_gpu_status=$null}
    try {
        $state.collector_process=[System.Diagnostics.Process]::GetCurrentProcess()
        $file=[System.IO.FileStream]::new($Path,[System.IO.FileMode]::CreateNew,[System.IO.FileAccess]::Write,[System.IO.FileShare]::Read)
        $state.writer=[System.IO.StreamWriter]::new($file,[System.Text.UTF8Encoding]::new($false));$state.writer.AutoFlush=$true;$state.segments.Add([System.IO.Path]::GetFileName($Path))
        $state.sampler=[LastWorldHardwareTelemetry]::new([int]$Owner.process_id,[long]$Owner.start_utc_ticks,$IncludeGpu)
    } catch {$state.error=$_.Exception.Message;$state.status='unavailable'}
    finally {$watch.Stop();$state.setup_ms=$watch.Elapsed.TotalMilliseconds}
    return $state
}
function Sample-LastWorldHardwareTelemetry($State,$Process,[double]$ElapsedSeconds) {
    if ($null -eq $State -or $null -eq $State.writer) {return}
    $watch=[System.Diagnostics.Stopwatch]::StartNew();$collectorCpuBefore=$null
    try {$collectorCpuBefore=$State.collector_process.TotalProcessorTime.TotalMilliseconds} catch {}
    try {
        $data=$null
        if ($null -ne $State.sampler) {$data=$State.sampler.Sample($Process)}
        $collectionMs=$watch.Elapsed.TotalMilliseconds
        $row=[ordered]@{schema_version=1;timestamp_utc=[DateTime]::UtcNow.ToString('o');elapsed_seconds=$ElapsedSeconds;owner=$State.owner;status=$State.status;error=$State.error;hardware=$data;collection_ms=$collectionMs;overhead_note='Supervisor collection/serialization/write is separate from native frame timing; previous sample total is reported separately';previous_sample_collector_cpu_ms=$State.last_collector_cpu_ms;previous_sample_and_log_ms=if ($State.samples -eq 0) {$null} else {$State.last_sample_ms}}
        $line=$row|ConvertTo-Json -Depth 12 -Compress
        if ($State.schema_probe -eq 'not_run') {
            $probe=$line|ConvertFrom-Json
            if ($probe.schema_version -ne 1 -or [int]$probe.owner.process_id -ne [int]$State.owner.process_id -or $null -eq $probe.PSObject.Properties['hardware']) {throw 'Hardware JSON schema/owner probe failed.'}
            $State.schema_probe='passed'
        }
        $lineBytes=[System.Text.Encoding]::UTF8.GetByteCount($line)+[System.Text.Encoding]::UTF8.GetByteCount([Environment]::NewLine)
        if ($State.segment_bytes -gt 0 -and $State.segment_bytes+$lineBytes -gt $State.segment_limit_bytes) {
            $State.writer.Dispose();$State.writer=$null;$State.segment_index++
            $segment=Join-Path ([System.IO.Path]::GetDirectoryName($State.path)) ([System.IO.Path]::GetFileNameWithoutExtension($State.path)+'.segment-'+$State.segment_index.ToString('D4')+'.jsonl')
            $file=[System.IO.FileStream]::new($segment,[System.IO.FileMode]::CreateNew,[System.IO.FileAccess]::Write,[System.IO.FileShare]::Read)
            $State.writer=[System.IO.StreamWriter]::new($file,[System.Text.UTF8Encoding]::new($false));$State.writer.AutoFlush=$true
            $State.segments.Add([System.IO.Path]::GetFileName($segment));$State.segment_bytes=0L
        }
        $State.writer.WriteLine($line);$State.segment_bytes+=$lineBytes;$State.samples++
        if ($null -ne $data) {
            if ($data.gpu.valid_engine_count -gt 0) {$State.valid_gpu_samples++}
            if ($data.cpu.process_status -eq 'sampled') {$State.valid_cpu_samples++}
            if ($data.memory.status -eq 'sampled') {$State.valid_ram_samples++}
            $State.last_gpu_status=$data.gpu.status
            $maxima=@{process_cpu_one_logical_cpu_max_percent=$data.cpu.process_percent_one_logical_cpu;process_cpu_logical_capacity_max_percent=$data.cpu.process_percent_reported_logical_capacity;system_cpu_max_percent=$data.cpu.system_busy_percent;gpu_busiest_engine_max_percent=$data.gpu.busiest_engine_percent;gpu_3d_max_percent=$data.gpu.engine_3d_max_percent;gpu_compute_max_percent=$data.gpu.engine_compute_max_percent;gpu_copy_max_percent=$data.gpu.engine_copy_max_percent;gpu_dedicated_process_max_bytes=$data.gpu.dedicated_process_bytes;gpu_shared_process_max_bytes=$data.gpu.shared_process_bytes}
            foreach ($key in $maxima.Keys) {if ($null -ne $maxima[$key]) {$State.extrema[$key]=if ($null -eq $State.extrema[$key]) {$maxima[$key]} else {[Math]::Max([double]$State.extrema[$key],[double]$maxima[$key])}}}
            if ($null -ne $data.memory.available_physical_bytes) {$State.extrema.system_available_physical_min_bytes=if ($null -eq $State.extrema.system_available_physical_min_bytes) {$data.memory.available_physical_bytes} else {[Math]::Min([double]$State.extrema.system_available_physical_min_bytes,[double]$data.memory.available_physical_bytes)}}
        }
    } catch {$State.error=$_.Exception.Message;$State.status='unavailable_after_sample_error'}
    finally {
        try {if ($null -ne $collectorCpuBefore) {$cpuMs=$State.collector_process.TotalProcessorTime.TotalMilliseconds-$collectorCpuBefore;if ($cpuMs -ge 0) {$State.last_collector_cpu_ms=$cpuMs;$State.collector_cpu_ms_total=if ($null -eq $State.collector_cpu_ms_total) {$cpuMs} else {$State.collector_cpu_ms_total+$cpuMs};$State.collector_cpu_ms_max=if ($null -eq $State.collector_cpu_ms_max) {$cpuMs} else {[Math]::Max($State.collector_cpu_ms_max,$cpuMs)}}}} catch {}
        $watch.Stop();$State.last_sample_ms=$watch.Elapsed.TotalMilliseconds;$State.sample_and_log_ms_total+=$State.last_sample_ms;$State.sample_and_log_ms_max=[Math]::Max($State.sample_and_log_ms_max,$State.last_sample_ms)}
}
function Stop-LastWorldHardwareTelemetry($State) {
    if ($null -eq $State) {return $null}
    try {if ($null -ne $State.sampler) {$State.sampler.Dispose()}} catch {$State.error=$_.Exception.Message}
    finally {if ($null -ne $State.writer) {try {$State.writer.Dispose()} catch {$State.error=$_.Exception.Message;$State.status='unavailable_log_close'}}}
    if ($null -ne $State.collector_process) {$State.collector_process.Dispose()}
    $status=$State.status;if ($status -eq 'sampling') {$status=if ($State.samples -eq 0) {'no_live_samples_before_child_exit'} else {'recorded_check_metric_statuses'}}
    return @{schema_version=1;status=$status;gpu_requested=$State.gpu_requested;segments=$State.segments.ToArray();segment_limit_bytes=$State.segment_limit_bytes;extrema=$State.extrema;samples=$State.samples;valid_gpu_engine_samples=$State.valid_gpu_samples;valid_cpu_delta_samples=$State.valid_cpu_samples;valid_system_ram_samples=$State.valid_ram_samples;last_gpu_status=$State.last_gpu_status;schema_probe=$State.schema_probe;setup_ms=$State.setup_ms;collector_process_cpu_ms_total=$State.collector_cpu_ms_total;collector_process_cpu_ms_max=$State.collector_cpu_ms_max;overhead_note='Stopwatch wall collection/format/write elapsed; collector CPU is supervisor-process delta during each sample, OS resolution applies; both separate from native frame time';sample_and_log_ms_total=$State.sample_and_log_ms_total;sample_and_log_ms_max=$State.sample_and_log_ms_max;error=$State.error;sampling_interval_seconds=1;per_counter_instance_limit=512;per_counter_buffer_limit_bytes=1048576;gpu_sampling_budget_ms=250;synchronous_PDH_call_cannot_be_interrupted=$true;process_RAM_source='attempt peak_working_set_bytes/peak_private_bytes and pc-host-history owned_process_memory events';performance_qualification=$false}
}
