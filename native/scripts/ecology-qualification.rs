//! Standalone CPU ecological diagnostics. Compile with the existing pinned serde
//! dependencies; no graphics device, tactical population or performance claim.
#[path = "../src/climate.rs"]
mod climate;
#[path = "../src/ecology.rs"]
mod ecology;
use ecology::{Ecology, EcologyPatch};
fn main() {
    if let Err(error) = run() {
        eprintln!("ECOLOGY_QUALIFICATION_FAILED: {error}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let seed = args
        .get(1)
        .map_or(Ok(78421), |v| v.parse::<u32>().map_err(|e| e.to_string()))?;
    let path = args.get(2);
    let mut env: Vec<EcologyPatch> = if let Some(path) = path {
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        vec![EcologyPatch::default(); 12]
    };
    let width = args.get(3).map_or(Ok(env.len()), |v| {
        v.parse::<usize>().map_err(|e| e.to_string())
    })?;
    let through = args
        .get(4)
        .map_or(Ok(72_000), |v| v.parse::<u64>().map_err(|e| e.to_string()))?;
    let mut eco = Ecology::new(seed, env.len())?;
    eco.set_patch_width(width)?;
    eco.accept_initial_habitat(&env)?;
    println!(
        "{}",
        serde_json::json!({"phase":"initial","seed":seed,"tick":0,"ecology":eco.census()})
    );
    for tick in (20..=through).step_by(20) {
        for (p, e) in env.iter_mut().enumerate() {
            let c = climate::SeasonState::at_tick(tick, seed, p as u32);
            e.temperature_milli_c = c.temperature_milli_c;
            e.moisture_permille = (eco.root_zones[p].shallow_water / 20).min(1000) as u16;
            eco.add_rain(p as u32, c.rainfall_liters_per_patch_second as u64 * 100)?;
        }
        eco.step(tick, &env)?;
        if tick % 1200 == 0 {
            println!(
                "{}",
                serde_json::json!({"phase":"checkpoint","seed":seed,"tick":tick,"ecology":eco.census()})
            );
        }
    }
    eco.validate()?;
    Ok(())
}
