fn main() {
    if let Err(error) = prepare_data_directory() {
        eprintln!("LAST WORLD startup failed: {error}");
        std::process::exit(1);
    }

    last_world_native::logging::install();
    last_world_native::logging::log("HOST", &last_world_native::diagnostics::host().to_string());
    last_world_native::logging::log(
        "LAUNCH",
        &serde_json::json!({"arguments":std::env::args().skip(1).collect::<Vec<_>>()}).to_string(),
    );
    if let Err(error) = run() {
        last_world_native::logging::log("FATAL", &error);
        std::process::exit(1);
    }
}

fn data_directory(
    supplied: Option<&std::ffi::OsStr>,
    executable: Option<&std::path::Path>,
) -> Result<Option<std::path::PathBuf>, String> {
    if let Some(supplied) = supplied {
        let folder = std::path::Path::new(supplied);
        if !folder.is_absolute() {
            return Err("LAST_WORLD_DATA_DIR must be an absolute path".into());
        }
        return Ok(Some(folder.to_owned()));
    }
    Ok(executable.and_then(|p| p.parent()).map(|p| p.to_owned()))
}

fn prepare_data_directory() -> Result<(), String> {
    let supplied = std::env::var_os("LAST_WORLD_DATA_DIR");
    let executable = std::env::current_exe().ok();
    if let Some(folder) = data_directory(supplied.as_deref(), executable.as_deref())? {
        if supplied.is_some() {
            std::fs::create_dir_all(&folder)
                .map_err(|e| format!("Cannot create data directory {}: {e}", folder.display()))?;
            std::env::set_current_dir(&folder)
                .map_err(|e| format!("Cannot enter data directory {}: {e}", folder.display()))?;
        } else {
            // Preserve direct launches of the existing executable beside its data.
            let _ = std::env::set_current_dir(folder);
        }
    }
    Ok(())
}

fn counts(raw: &str) -> Result<Vec<u32>, String> {
    raw.split(',')
        .map(|value| {
            let n: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("Invalid agent count: {value:?}"))?;
            if n == 0 || n as usize > last_world_native::gpu::CAPACITY {
                return Err(format!(
                    "Agent count {n} is outside 1..={}; counts are never silently reduced",
                    last_world_native::gpu::CAPACITY
                ));
            }
            Ok(n)
        })
        .collect()
}

fn seconds(raw: &str) -> Result<f64, String> {
    let n: f64 = raw
        .parse()
        .map_err(|_| format!("Invalid duration: {raw:?}"))?;
    if !n.is_finite() || n <= 0. {
        return Err("Duration must be finite and greater than zero".into());
    }
    Ok(n)
}

fn speed(raw: &str) -> Result<last_world_native::clock::SimulationSpeed, String> {
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("Invalid speed {raw:?}; use 1, 2 or 4"))?;
    last_world_native::clock::SimulationSpeed::try_from(value).map_err(|error| error.to_string())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--headless" | "--check" | "--describe" | "--campaign" | "--campaign-check"
            | "--campaign-report" | "--river" | "--water-report" | "--ground-report"
            | "--river-check" | "--benchmark-list" => i += 1,
            "--counts" | "--seconds" | "--capture" | "--backend" | "--speed"
            | "--benchmark-case" | "--benchmark-output" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    return Err(format!("{} requires a value", args[i]));
                }
                i += 2;
            }
            other => return Err(format!("Unknown option: {other}")),
        }
    }
    let value = |key: &str| {
        args.iter()
            .position(|s| s == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let speed = speed(value("--speed").as_deref().unwrap_or("1"))?;
    if args.iter().any(|x| x == "--describe") {
        if args.iter().any(|x| {
            matches!(
                x.as_str(),
                "--benchmark-list" | "--benchmark-case" | "--benchmark-output"
            )
        }) {
            return Err("--describe cannot be combined with a benchmark case/list".into());
        }
        let scope = last_world_native::diagnostics::scope();
        last_world_native::logging::log("SCOPE", &scope.to_string());
        println!(
            "{}",
            serde_json::to_string_pretty(&scope).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let backend = match value("--backend").as_deref().unwrap_or("auto") {
        "auto" => None,
        "vulkan" => Some(wgpu::Backends::VULKAN),
        "dx12" => Some(wgpu::Backends::DX12),
        other => {
            return Err(format!(
                "Unsupported backend {other:?}; use auto, vulkan or dx12"
            ));
        }
    };
    let benchmark_list = args.iter().any(|a| a == "--benchmark-list");
    let benchmark_case = value("--benchmark-case");
    if benchmark_list || benchmark_case.is_some() || value("--benchmark-output").is_some() {
        if args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--headless"
                    | "--check"
                    | "--describe"
                    | "--campaign"
                    | "--campaign-check"
                    | "--campaign-report"
                    | "--river"
                    | "--water-report"
                    | "--ground-report"
                    | "--river-check"
                    | "--counts"
                    | "--seconds"
                    | "--capture"
                    | "--speed"
            )
        }) {
            return Err("Benchmark cases own their requested rate and workload; normal game/fixture options cannot be combined".into());
        }
        if benchmark_list {
            if args.len() != 2 {
                return Err("Use --benchmark-list alone".into());
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&last_world_native::campaign_benchmark::plan())
                    .map_err(|e| e.to_string())?
            );
            return Ok(());
        }
        let id = benchmark_case.ok_or("--benchmark-output requires --benchmark-case")?;
        let output =
            value("--benchmark-output").ok_or("--benchmark-case requires --benchmark-output")?;
        let spec = last_world_native::campaign_benchmark::case(&id)
            .ok_or_else(|| format!("Unknown benchmark case {id:?}"))?;
        let output = std::path::Path::new(&output);
        if spec.mode == last_world_native::campaign_benchmark::BenchmarkMode::Window {
            return pollster::block_on(last_world_native::play::benchmark_windowed(
                &id, output, backend,
            ));
        }
        let report = last_world_native::campaign_benchmark::run_cpu_case(&id, output)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        if report["passed"] != true {
            return Err("Benchmark case did not complete with valid state; report retained".into());
        }
        return Ok(());
    }
    if args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--campaign" | "--campaign-check" | "--campaign-report"
        )
    }) {
        if args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--headless"
                    | "--river"
                    | "--river-check"
                    | "--ground-report"
                    | "--water-report"
                    | "--counts"
                    | "--capture"
                    | "--seconds"
                    | "--check"
            )
        }) {
            return Err("Campaign and isolated fixture options cannot be combined".into());
        }
        if args.iter().any(|a| a == "--campaign-report") {
            let report = last_world_native::campaign_scenarios::integration_report()?;
            last_world_native::logging::log("CAMPAIGN_CHECK", &report.to_string());
            use std::io::Write;
            let mut history = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("campaign-history.jsonl")
                .map_err(|e| e.to_string())?;
            serde_json::to_writer(&mut history, &report).map_err(|e| e.to_string())?;
            history.write_all(b"\n").map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            if report["passed"] != true {
                return Err(
                    "Connected campaign correctness scenario failed; inspect console.txt".into(),
                );
            }
            return Ok(());
        }
        return pollster::block_on(last_world_native::play::windowed(
            backend,
            speed,
            args.iter().any(|a| a == "--campaign-check"),
        ));
    }
    let headless = args.iter().any(|x| x == "--headless");
    let automated_river = args.iter().any(|x| x == "--river-check");
    let river_fixture = automated_river || args.iter().any(|x| x == "--river");
    if args
        .iter()
        .any(|x| x == "--water-report" || x == "--ground-report")
    {
        if ["--counts", "--seconds", "--capture", "--check"]
            .iter()
            .any(|key| args.iter().any(|arg| arg == key))
        {
            return Err("--water-report uses a named fixed CPU water scene; population/duration/capture benchmark flags do not apply".into());
        }
        if headless || river_fixture {
            return Err("--water-report is a separate CPU correctness scene; use it without --river or --headless".into());
        }
        let report = last_world_native::river::interaction_report()?;
        if report["conserved"].as_bool() != Some(true) {
            return Err("Coupled ground/water report did not conserve its stocks".into());
        }
        last_world_native::logging::log("GROUND_WATER_CHECK", &report.to_string());
        use std::io::Write;
        let mut history = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("river-history.jsonl")
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut history, &report).map_err(|e| e.to_string())?;
        history.write_all(b"\n").map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if river_fixture && headless {
        return Err("--river is an interactive water fixture; it cannot label a population benchmark as a coupled water test".into());
    }
    if !headless
        && ["--counts", "--seconds", "--capture", "--check"]
            .iter()
            .any(|key| args.iter().any(|arg| arg == key))
    {
        return Err("--counts, --seconds, --capture and --check require --headless".into());
    }
    if headless {
        let counts =
            counts(&value("--counts").unwrap_or("1000,5000,10000,25000,50000,100000".into()))?;
        let seconds = seconds(&value("--seconds").unwrap_or("5".into()))?;
        pollster::block_on(last_world_native::runtime::headless(
            counts,
            seconds,
            value("--capture"),
            args.iter().any(|x| x == "--check"),
            backend,
            speed,
        ))
    } else {
        if automated_river {
            pollster::block_on(last_world_native::runtime::river_check(backend, speed))
        } else {
            pollster::block_on(last_world_native::runtime::windowed_options(
                backend,
                speed,
                river_fixture,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_directory_uses_absolute_override_or_original_executable_parent() {
        let root = std::env::temp_dir().join("last-world-path-resolution");
        let executable = root.join("App").join("last-world.exe");
        let data = root.join("Data");
        assert_eq!(
            data_directory(Some(data.as_os_str()), Some(&executable)).unwrap(),
            Some(data.clone())
        );
        assert_eq!(
            data_directory(None, Some(&executable)).unwrap(),
            Some(root.join("App"))
        );
        assert_eq!(data_directory(None, None).unwrap(), None);
        assert_eq!(
            data_directory(Some(data.as_os_str()), None).unwrap(),
            Some(data)
        );
    }
    #[test]
    fn data_directory_rejects_relative_or_empty_override_before_startup() {
        for supplied in ["", ".", "Data", "../Data", "./Data"] {
            assert!(data_directory(Some(std::ffi::OsStr::new(supplied)), None).is_err());
        }
    }
    #[test]
    fn counts_reject_invalid_input_without_reducing_requested_workload() {
        for value in ["", "0", "-1", "100001", "1000,bad", "1000,"] {
            assert!(counts(value).is_err(), "accepted {value:?}");
        }
        assert_eq!(counts("10000,100000").unwrap(), vec![10000, 100000]);
    }
    #[test]
    fn duration_rejects_nonfinite_and_nonpositive_values() {
        for value in ["NaN", "inf", "-1", "0", "bad"] {
            assert!(seconds(value).is_err());
        }
        assert_eq!(seconds("0.5").unwrap(), 0.5);
    }
    #[test]
    fn speed_accepts_only_explicit_testing_options() {
        for raw in ["0", "3", "8", "32", "NaN", "bad"] {
            assert!(speed(raw).is_err());
        }
        for value in [1, 2, 4] {
            assert_eq!(speed(&value.to_string()).unwrap().multiplier(), value);
        }
    }
}
