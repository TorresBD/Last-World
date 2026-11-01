//! Describes current executable scope without allocating a world or GPU device.
//! Targets are kept separate from implemented benchmark dimensions/counts.

pub fn host() -> serde_json::Value {
    serde_json::json!({
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "available_logical_parallelism": std::thread::available_parallelism().ok().map(usize::from),
        "process_id": std::process::id(),
        "physical_cpu_model": null,
        "process_ram_bytes": null,
        "driver_vram_usage_bytes": null
    })
}

pub fn scope() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 2,
        "engine": crate::engine_identity(),
        "scope_mode": "executable_inventory_not_current_run",
        "workload_kind": "isolated_benchmarks_and_small_native_campaign",
        "integrated_performance_qualified": false,
        "host": host(),
        "current_benchmark": {
            "map_meters": [crate::terrain::WW, crate::terrain::HH],
            "terrain_cell_meters": 8,
            "visual_pixel_meters": 2,
            "agent_capacity": crate::gpu::CAPACITY,
            "agent_record_bytes": std::mem::size_of::<crate::gpu::Agent>(),
            "agent_kind": "movement_records_not_implemented_military_civilian_or_animal_rules",
            "nominal_ticks_per_game_second": crate::clock::TACTICAL_HZ,
            "clock_authority": "cpu_integer_ticks_with_retained_backlog",
            "selectable_test_speeds": [1, 2, 4],
            "season_cycle_ticks": crate::clock::SEASON_CYCLE_TICKS,
            "season_environment_effects_implemented": false,
            "headless_time_policy": "at_least_50ms_synthetic_frame_delta; synchronized throughput is not a real-time simulation-speed qualification",
            "cpu_authoritative_editable_terrain": true,
            "editable_terrain_resolution_meters": crate::terrain::CELL_METERS,
            "sparse_fine_terrain_streaming_and_campaign_edit_jobs": false,
            "partial_cell_and_ground_texture_updates": true,
            "combat_logistics_ecology_and_campaign_saves": false
        },
        "optional_river_fixture": {
            "command": "--river",
            "cpu_correctness_command": "--water-report",
            "water_cells": crate::river::WIDTH * crate::river::HEIGHT,
            "water_cell_meters": crate::terrain::CELL_METERS,
            "cpu_owned_finite_volume": true,
            "finite_soil_sediment_and_vegetation_transport": true,
            "soil_water_air_exposure_delay_and_drying": true,
            "brown_depth_darkened_bed": true,
            "automated_test_command": "--river-check --speed 4",
            "batched_pc_runner": "Run-Checks.cmd",
            "single_feedback_file": "Send-Back.zip",
            "dam_and_breach_controls": "G close / J open; changing barrier preserves volume",
            "coupled_military_ecology_or_navies": false,
            "whole_20km_world_hydrology": false
        },
        "native_campaign": {
            "command": "--campaign",
            "correctness_report": "--campaign-report",
            "automatic_render_check": "--campaign-check --speed 4",
            "map_meters": [4800, 3200],
            "default_military": 10000,
            "default_workers": 2000,
            "additional_living_wildlife": true,
            "gameplay_authority": "cpu_integer_20Hz_clock",
            "renderer": "same_wgpu_device_with_campaign_display_copies",
            "additional_mechanism_source_count": 30,
            "mandatory_biology_is_additional": true,
            "full160_or_production20km": false,
            "actual_PC_qualification_for_this_build": "requires_matching_returned_batch",
            "scope_record": "project-records/STATE.md"
        },
        "approved_release_targets": {
            "large_map_meters": [20000, 20000],
            "military_pawns": 10000,
            "additional_non_military_and_wildlife_required": true,
            "large_war_hours": [3, 4],
            "first_slice_additional_traits": 30,
            "mandatory_biology_is_additional_to_that_count": true,
            "full_additional_catalogue": 160,
            "platform": "native_pc",
            "combat_navy_bridges_docks_and_transports": true,
            "terrain_engineering_traits_dams_and_branching_burrows": true
        }
    })
}
