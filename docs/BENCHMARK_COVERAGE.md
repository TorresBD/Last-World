# Current speed-test revision — 2026-10-30

Tor requested one gradual speed test with adequate loading time and no pointless faster-speed retries after a clear performance ceiling. The current compiled plan therefore has12cases: all16separate CPU/window rate cases are replaced by ONE native `window-speed-ramp` process, one evolving world and one existing GPU device. The prior27-case coverage/inventory discussion below is historical evidence for the earlier6bac0962/2c367d4d build; it does not describe the current speed schedule.

Native setup/world loading finishes before the session starts. The live world then receives15wall seconds at1x as initial warm-up. Each increase receives5seconds of live settling. Rate1/2/4/8/16/32/64/100 is considered in order; each measured stage needs at least two10-second windows. Two consecutive windows with growing backlog and <98% achieved speed, or <90% achieved speed, stop higher requests. Mixed evidence gets at most four10-second windows; a rate that still cannot qualify stops the ramp rather than trying a faster rate. A qualified stage needs >=98% of requested speed, <=4ticks growing debt and <=4pending ticks at both endpoints. Inherited backlog cannot manufacture a qualified rate. This is a measurement tolerance, not a promise of full-game performance.

Each stage records actual fixed-tick and complete-frame quantiles, requested/achieved rate, actual ticks, exact pending debt and individual windows. `speed_ramp.highest_sustained_rate` is nullable; `remaining_rates_not_run` and the completion reason are explicit. A capacity-limited ramp can complete its correctness protocol while `requested_rate_sustained` is false. No skipped higher rate is treated as tested or passed. Whole-session diagnostics mix warm-up, settling and changing rates and must not be used as per-rate measurements. Stage samples exclude boundary-crossing frames and terminal captures; no GPU timing is invented.

Normal1x/2x/4x gameplay,20Hzrules,64benchmark steps/call, live populations and clock debt are unchanged. Every transition settles its processing tail at the old rate and subtracts that precharged time from the next input exactly once. Warm-up runs biology, water, construction and combat normally. No world freeze, rescue spawning or free completion is introduced.

The seasonal correctness case retains72,000ADDITIONALactual ticks in its own fresh group. Large-population, alternate-seed and recovery-control cases remain focused correctness/load checks, not additional speed comparisons. The supervisor still gives each actual case at most two attempts and continues after failure; a measured capacity stop does not request a redundant retry.

---

# Native benchmark coverage and Windows harness audit

2026-10-30. This document maps executable mechanisms, available tests, and natural connected evidence. It is an inventory and qualification plan, not a claim that every feature, machine, trait combination, requested speed or GPU backend has passed.

## Current truth

The last completed frozen host batch passed **214 library tests and 3 CLI tests**, with no ignored tests. Its raw output is `evidence/milestone-b-integration-2026-10-30/final-custody-all-targets-checks.log`. The complete names below were obtained by executing those existing host test binaries with `--list`; no test was rerun and no compilation was performed for this document. This is a **host-cfg inventory, not an executed Windows inventory**. The new benchmark-clock/soak work changes source after that batch. Regenerate the expected list from the newly built frozen host harness, enumerate the actual Windows harness on the player's PC, and record the differences. Never hard-code 217 as the next package's total.

The final frozen **small connected CPU** scenario passed all 14 strict gates at gameplay tick 17,280 and completed its late save/load comparison through tick 17,320. It used an explicit 512 military/64 worker correctness fixture plus the real wildlife ledger on the resident 4.8 × 3.2 km map. It preserved the ordinary game defaults of 10,000 military and 2,000 workers. Evidence is `final-connected-report.json`, `final-connected-identity.json` and `final-connected-stdout.txt` in the same evidence folder. That report is distinct from every failed/intermediate run and does not certify the new benchmark driver or later source.

One separate ecological diagnostic processed the actual 600-patch starting habitat through 72,000 ticks, preserving all 14,800 founders at generation and keeping all eight fauna species extant. Its water depths/connectivity were an initial snapshot, rather than a continuously evolving military/river world. `ecology-final-unwarred-season-summary.json` records that limitation and its source hashes. It cannot substitute for the proposed full connected seasonal soak.

Only physical trait commits **46 and 65** were naturally recorded by the final connected report (`final.physical.committed_traits = {"46":2,"65":5}`). Other selected traits have mechanism fixtures and may evaluate during ordinary simulation, but the existing aggregate counters do not individually establish their natural consequences. `biology_events`, a nonempty trait mask, a generated species, or a queued intent is not a successful trait-effect counter.

No usable GPU adapter was available on the development host. Windows compilation/import inspection, CPU fixtures, font rasterization and WGSL validation do not prove Windows startup, native input, GPU behavior, frame rate or a long war. The production 20 × 20 km world, other 130 additional traits, complete mandatory biology, complete navy, sanctuary system, mutation and full causal ecological verdict remain distinct scope.

## Reading coverage statuses

Use two independent axes; never collapse them into a single percentage of game completion.

| Status | Meaning |
|---|---|
| `tested_mechanism_fixture` | The exact named test exercises a finite mechanism, failure, or interaction fixture. The recorded host batch passed that compiled snapshot. Test setup may deliberately arrange a condition; it is not a claim that natural AI reached it. |
| `observed` / integrated natural observation | A named connected run recorded the actual event/state transition under ordinary rules, with its real counts, source identity and prerequisites. The scope is only the observed run. |
| `not_reached` | A supported mechanism has no sufficient natural evidence in this run. It may have executed without a dedicated metric; this label is not proof of absence. Retain the reason and any partial state evidence. |
| `planned` / `unimplemented` | There is no implemented mechanism or sufficient completed integration to exercise. The proposed driver uses `unimplemented` in machine-readable coverage; this document uses `planned` for retained future scope. A test of a neighboring mechanism cannot promote this status. |
| qualified performance | Requires actual hardware results, achieved throughput, frame-time/debt/memory data and the actual workload. None of the status labels above implies it. |

The five behavior-helper tests appear under both `ecology::behavior::tests` and `ecology_behavior::tests` in the current library harness. These are ten executable test names but five helper properties, not ten independent biological mechanisms. Execute every discovered name; do not inflate feature coverage by counting duplicate inclusion.

## Subsystem map

All test references are exact libtest names. More exhaustive names are listed in the inventory appendix. The natural evidence paths in this table refer to `final-connected-report.json` unless explicitly marked as a proposed soak field.

| Implemented subsystem / actual scope | Available exact mechanism/interaction tests | Natural runtime evidence and present boundary |
|---|---|---|
| Authoritative integer clock; 1×/2×/4× gameplay; bounded per-frame debt; pause | `clock::tests::partitioning_wall_time_does_not_change_ticks_at_any_allowed_speed`; `clock::tests::bounded_work_retains_all_debt_and_pause_neither_accrues_nor_drains_it`; `clock::tests::speed_changes_do_not_reprice_previous_debt_or_fractional_time`; `runtime::clock_integration_tests::four_x_control_retains_unclamped_time_and_pause_preserves_owed_ticks`; CLI `tests::speed_accepts_only_explicit_testing_options` | `final.tick`, `final.pending_ticks`, `final.requested_speed`; the successful small case is requested 1×. Actual integrated 2×/4× throughput is not reached by these assertions. New `advance_benchmark` rate/debt tests are source additions awaiting a new harness inventory. Requested 100× is a diagnostic clock demand, never an achieved-rate or playable-speed claim. |
| Procedural ground/foliage reference, seed reproducibility and local clearing | `terrain::tests::matches_long_front_reference`; `terrain::tests::removing_foliage_clears_existing_grown_pixels_locally_to_brown_soil`; `terrain::tests::ground_cache_rejects_regrowth_and_invalid_state_without_committing` | Living-plant and ground-cover stocks appear in `final.ecology` / `final.water.ground`; no pixel-style or procedural-appearance acceptance follows from CPU stock checks. Grown atlas assets are embedded/reused. Native screenshots and player review remain separate. |
| Height deformation, craters, signed construction cut/fill, smoothing, dirty upload regions | `terrain::tests::radial_cut_is_local_and_an_invalid_edit_changes_no_elevation`; `terrain::tests::signed_elevation_edits_report_exact_coarse_volume_and_reject_invalid_targets`; `terrain::tests::smoothing_reads_the_original_neighborhood_before_committing`; `terrain::tests::dirty_ranges_are_deduplicated_disjoint_and_never_cross_rows`; `ground::tests::construction_cut_fill_transfers_exact_soil_without_moving_water` | `final.completed_jobs`, sediment/spoil/ground totals and natural construction gates establish bounded state changes. Per-explosion GPU upload cost, appearance and shadow quality require native timing/capture; dirty CPU ranges alone do not measure a GPU transfer. |
| Physical construction, reservations, pickup, site access, material consumption and cancellation recovery | `construction::tests::jobs_require_real_pickup_delivery_and_work_then_consume_once`; `construction::tests::cancellation_at_site_needs_physical_material_recovery_before_refund`; `construction::tests::one_worker_walks_out_after_a_solid_build_and_completes_a_second`; `construction::tests::suspended_hauler_keeps_physical_load_and_job_then_resumes_delivery` | `gates.paid_worker_construction`, `final.completed_jobs`, `unfinished_job_diagnostics`; actual jobs complete in the final connected case. This is not proof that every construction kind or destruction/cancellation combination occurred naturally. |
| Finite mining and physical ore return | `construction::tests::completed_solid_mine_loads_at_exterior_and_worker_hauls_real_ore_to_base`; `construction::tests::reservation_cancel_and_save_roundtrip_preserve_exact_resources` | `gates.mining_and_delivery`, `final_mining_diagnostics`, finite deposits/stores and actual `ConstructionEvent::Mined` → `CampaignData.ore_delivered`. Exterior access fixes the blocked solid-mine center. Warehouses are not credited before real return. |
| Economy, timber/food production and care supplies | `construction::tests::jobs_require_real_pickup_delivery_and_work_then_consume_once`; `ecology::tests::capture_care_research_breeding_release_are_real_finite_transfers`; `ecosystem_physics::tests::real_nested_construction_transfers_finite_biomass_and_survives_json_save` | Mills/farms call finite `Ecology::export_biomass`; faction stores/base water fund care. `final_capture_diagnostics.water_given_to_care` and per-captive `care_food`/`care_water` establish actual care in the small case. There is no standalone naturally observed factory supply-chain/economic balance qualification for every building. |
| Shared navigation, topology versus costs, bounded incremental search and saveable deadlines | `tactics::tests::shared_route_has_one_build_and_flood_invalidates_it`; `tactics::tests::changing_costs_do_not_starve_search_or_interrupt_live_cost_refresh`; `tactics::tests::partial_navigation_jobs_survive_save_without_changing_next_ticks`; `tactics::tests::cost_refresh_frontier_and_deadline_survive_save_exactly` | `final.tactics.route_builds`, `route_cache_hits`, `route_cost_refreshes`, `route_invalidations`, `route_node_expansions`, real handler/hauler progress. A cache hit is not proof of a usable final route; positions/arrival/work progress must accompany it. |
| Crowds, collision, exact army/worker counts and wet-jungle haulers | `tactics::tests::ten_thousand_military_plus_two_thousand_workers_keep_exact_counts`; `tactics::tests::shared_corridor_keeps_six_haulers_moving_through_a_dense_worker_spawn`; `tactics::tests::terrain_derived_wet_jungle_corridor_keeps_physical_haulers_progressing`; `construction::tests::two_thousand_workers_keep_ids_and_only_assigned_reachable_workers_act` | The 12,000-pawn fixture advances only 20 tactical ticks. The final connected case is 512/64. Proposed soak `actual_counts`, process memory, debt and achieved rate must verify the full 10,000+2,000+wildlife workload without silent caps. |
| Military combat, finite ammunition, supply and deaths | `tactics::tests::combat_commits_real_casualties_and_conserves_ammunition`; `tactics::tests::retreat_and_new_solid_obstruction_do_not_leave_soldiers_frozen_on_old_targets`; `faction_support::tests::supply_requires_local_physical_arrival_and_caps_actual_acceptance` | `gates.actual_combat`, `final.tactics.fired_ammunition`, `deaths`, `lost_ammunition`, `supplied_ammunition`, ship health. Combat is observed in the small case. Full-war AI/balance and all weapon variants are not qualified. |
| Paid turrets/local field support | `faction_support::tests::turret_hits_real_target_on_cooldown_with_finite_paid_rounds`; `faction_support::tests::blocked_line_spends_nothing_and_opening_wall_allows_fire`; `faction_support::tests::remaining_stock_is_lost_once_on_destruction_and_cannot_respawn` | `final.field_support.stocks`, `totals.fired_rounds`, `targets_killed`, `supplied_rounds`, `destroyed_rounds`, `conserved`. Inspect actual nonzero values; merely including this report does not mean turret fire occurred. |
| Roads, trenches, bridges, flood evacuation and naval clearance | `construction::tests::managed_trench_preserves_water_and_soil_transactions_survive_ticks`; `tactics::tests::a_new_road_eventually_changes_preference_without_discarding_live_field`; `tactics::tests::foot_paths_do_not_treat_roads_as_bridges_and_naval_checks_clearance`; `tactics::tests::bridge_destruction_removes_ghost_crossing_without_unchanged_overlay_churn`; `tactics::tests::flood_evacuation_and_drowning_are_real_saved_finite_state` | Natural construction/travel is observed, but the complete bridge/trench/naval interaction set is `not_reached` in the short report. Road-dependent mud cost is tested; a road never supplies a bridge across deep water. |
| Boat/transport roles, boarding/disembarkation and sinking | `tactics::tests::boarding_and_release_keep_every_identity_owned_once_through_save`; `tactics::tests::sinking_transport_and_repeat_damage_do_not_duplicate_passenger_deaths`; `ui_interaction::tests::rally_board_and_disembark_tools_never_issue_legacy_global_actions` | Initial navy/transport foundations exist. The final small case does not establish a natural fleet/boarding/dock/island-economy cycle. Full cargo/fleet/naval roster and balance remain planned. |
| Generated species, compatible food webs, finite plants/wildlife, offscreen cohorts | `ecology::tests::generated_food_web_and_anatomy_are_compatible_and_deterministic`; `ecology::tests::founding_habitat_acceptance_relocates_only_existing_counts_once`; `ecology::tests::cross_seed_population_water_material_ledgers_and_serialized_continuity`; `ecology::tests::extinction_never_recovers_from_generation_seed_without_surviving_stages` | `gates.generated_habitat_food_web`, `final.ecology.generation_accepted`, founder relocation count, wild/transit/captive/offspring/plant stocks, explicit `death_reasons`/`extinct_species`. Cohorts are not ECS entities. One initial catalogue plant profile is never founded in this seed; its extinct flag is not evidence that the world lost a previously living plant population. |
| Hidden wildlife, coarse migration and actual fine engineering paths | `ecology::tests::hidden_burrow_animals_cannot_be_discovered_captured_or_eaten_through_ground`; `ecology::tests::thirst_search_respects_actual_border_masks_and_never_drinks_remote_stock`; `ecology_navigation::tests::continuous_wall_and_later_opening_change_both_sides`; `ecosystem_physics::tests::coarse_materialization_uses_connected_real_habitat_and_known_animal_never_teleports` | `final.ecology.transit_animals`, physical actor positions/active progress and saved states. Fine worksite paths are real; macro migration uses shared-border masks, not a full per-animal internal route to every border. Re-closing an edge during an existing macro transit remains a cross-feature gap. |
| Temperament, social grouping, territory and threat response | `ecology_behavior::tests::docile_handling_pays_real_time_but_less_than_identical_wild_animal`; `ecology_behavior::tests::observed_threats_change_response_without_making_defensive_animals_immune`; `ecology_behavior::tests::projected_group_size_and_territory_change_destination_ranking` | `final_capture_diagnostics.orders[].required_handling_ticks` / `handling_work_ticks`; actual capture uses profile costs. Combat calls `alert_threat`. No existing per-reason migration counter independently proves social/territorial/flee decisions in the short run. |
| Discovery, capture, physical pens, paid care, research, breeding, offspring and release | `ecology::tests::physical_pen_delivery_capacity_reserves_offspring_and_loss_stops_care_without_deleting_animals`; `ecology::tests::separate_pens_do_not_allow_remote_breeding`; `ecology::tests::materialized_capture_transport_housing_release_preserves_identity_and_finite_pregnancy`; `ecosystem_physics::tests::verified_custody_handoff_preserves_wallet_and_counts_cancels_old_jobs_and_saves_exact_xy` | All matching `gates`, named `milestones`, `final_capture_diagnostics`, `released_existing_animal`, real `parent_id`/housing/care/reserves and actual release points. **Pen** is the implemented building name; it is not a complete sanctuary/protected-land management system. |
| Handler/captive mortality and exact custody cleanup | `campaign::custody_mortality_tests::killed_handler_releases_same_animal_wallet_and_detail_at_actual_carrier_position`; `campaign::custody_mortality_tests::actual_captive_mortality_cancels_return_and_both_release_phases_without_respawn` | Named `CAPTURE_HANDLER_LOST` / `CAPTURE_CARGO_LOST` console events and exact surviving identity/material fields are available. These failure cases are fixture-tested; the healthy short scenario does not naturally exercise them. Failed float-coordinate checks remain preserved. |
| Ecological damage attribution and victory | `campaign_habitats::tests::local_committed_cut_reveals_only_its_real_room_without_new_animals_or_score`; `campaign_habitats::tests::blast_breaches_only_near_dam_erodes_reef_and_preserves_water_and_material` protect local changes from inventing damage/stock; military tests verify casualty ownership | `final.attributed_ecological_damage` is observed ([1940,2990] in the final small report), `final.winner` remains null. Current outcome rules include ship destruction and an ecological-damage threshold. Full causal downstream responsibility, complete score weighting and the ecological-stalemate verdict are **not qualified** by this run; there is no dedicated complete ecological-verdict unit case in the listed harness. |
| Cameras, anonymous radar, rotating sweep, occlusion, satellite reveal and prewarm | `campaign_support::tests::ordinary_units_give_no_vision_and_saved_exploration_is_not_current_vision`; `campaign_support::tests::taller_solid_blocks_radar_and_camera_but_short_or_nonsolid_objects_do_not`; `campaign_support::tests::radar_echo_is_a_snapshot_not_hidden_live_tracking_and_expires`; `campaign_support::tests::circular_satellite_prewarm_precedes_visibility_and_team_passes_are_offset` | `gates.discovery`, observed-target capture diagnostics and existing recon masks/battery/echo state establish some actual observation. Current `Campaign::report` has no complete recon telemetry summary; the proposed soak must sample it. Native radar outline/fill, smooth circular reveal and prewarm rendering cost still require real GPU captures/timing. |
| Battery-powered unarmed camera trooper | `campaign_support::tests::battery_runs_out_recharges_at_real_base_and_duplicate_tick_cannot_charge_it_twice`; `campaign_support::tests::newly_spawned_camera_is_not_charged_for_time_before_it_existed` | `Recon::battery_status` and serialized sensor state are actual evidence sources. Natural depletion→base travel→recharge is not independently established by the short lifecycle gates. Mounted wall/building camera snapping remains a separate incomplete feature. |
| Surface water: reservoirs, dam leakage/breach, outlets and exact staged flow | `water::tests::dam_fills_gradually_throttles_downstream_and_inundates_connected_bank`; `water::tests::breach_opens_flow_but_does_not_teleport_or_delete_reservoir`; `water::tests::simultaneous_donors_cannot_spend_the_same_volume_twice`; `water::exchange_tests::cross_owner_exchange_is_exact_and_failing_batch_is_atomic` | `final.water.stored_liters`, `initial_liters`, `external_inflow_liters`, `boundary_outflow_liters`, named extraction/return totals and `conserved`. Flow advances in the connected scenario, but a naturally built animal dam/reservoir/breach sequence is not reached there. Dynamic water is confined to the managed river, not the entire resident/20km world. |
| Ground erosion, sediment, foliage washout, wet brown substrate and delayed drying | `ground::tests::erosion_and_washout_change_depth_without_creating_soil_or_biomass`; `ground::tests::soil_keeps_moisture_during_air_delay_then_dries_and_rewetting_resets_it`; `terrain::tests::drained_surface_keeps_wet_brown_state_until_owner_changes_moisture`; `ground::tests::material_snapshot_moves_only_one_connection_per_tick` | `final.water.ground` includes exact erosion/deposition, suspended/bed sediment, debris/foliage, soil-water, infiltration/evaporation and outflow totals. Stocks changing naturally is observed; each drying/washout/rewetting branch still needs its own event evidence in a soak/native case. |
| Living-root/cover/shade feedback, mixing and mud traction | `ground::tests::current_root_and_cover_loss_changes_erosion_without_moving_stocks_on_assignment`; `ground::tests::live_cover_roots_and_canopy_change_actual_infiltration_and_evaporation_conservatively`; `ecosystem_physics::tests::soil_mixing_changes_actual_root_nutrient_access_and_conserves_both_owners`; `tactics::tests::wet_soil_reduces_offroad_traction_while_roads_limit_the_mud_cost` | Actual `root_reinforcement_permille`, `ground_cover_permille`, `canopy_permille`, upper-layer porosity and nav moisture project into managed ground/path costs. Assignment itself never creates water/material. Aggregated `Campaign::report` does not isolate each causal response; a soak needs condition/delta samples. |
| Sparse habitat engineering, material wallets, timed labor, host/roof loss | `ecosystem_physics::tests::queued_builder_gets_no_free_work_and_only_real_elapsed_awake_ticks_fund_tunnel`; `ecosystem_physics::tests::same_cell_final_approach_moves_and_pays_time_across_water_changes_and_save`; `ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass`; `habitat::tests::engineering_replay_across_seeds_preserves_invariants_and_failed_actions_are_atomic` | `final.physical.committed_traits`, `active_progress`, materials received/returned, rejection/stall counters and `conserved`. Natural commits46/65 are observed; queuing ~9,932 optional intents at the small run's end is not completion or proof of scalable queue memory. See the 30-ID matrix. |
| Jungle seasons, rainfall/temperature, biology and ground forcing | `climate::tests::whole_year_is_integer_clock_deterministic_and_repeats_conditions`; `climate::tests::living_jungle_has_real_wet_dry_forcing_without_universal_snow`; `clock::tests::season_cycle_deadline_is_exact_and_scales_with_selected_speed`; dormancy/migration/roots/mud tests above | One full cycle is72,000 ordinary ticks/one simulated hour. The small final run ends at17,320, before a full cycle; the separate ecological one-cycle diagnostic is not a fully coupled seasonal world. Climate's named wet/dry phases and some biological quarter/half-cycle predicates are not the same interface; cross-season behavior alignment remains a soak/review gap. |
| Exact saves, previous backups, corrupt recovery and continuing routes/cohorts/recon | `campaign_support::tests::saves::atomic_save_preserves_previous_and_ignores_interrupted_pending_files`; `campaign_support::tests::saves::corrupt_current_requires_explicit_recovery_and_cannot_poison_previous`; `campaign_support::tests::saves::noninteger_state_checksum_uses_saved_bytes_without_float_reformatting`; `ui_interaction::tests::corrupt_layout_recovers_previous_and_next_save_preserves_both_originals` | `save_receipt` and `final_state_save_receipt` compare actual serialized states plus40 ordinary ticks. Sizes were66,529,833B and91,373,408B. These are measured correctness-fixture save sizes, not production save/load targets, Windows durability proof or long-war size limits. |
| Selection, bounded queued commands, UI information boundaries, layout/fonts/cache | `campaign_commands::tests::explicit_subset_moves_without_ordering_other_units_and_enemy_selection_is_atomic`; `campaign_commands::tests::shift_queue_and_hold_are_owned_saved_and_bounded_without_changing_current_goal`; `ui_interaction::tests::contextual_orders_are_selected_only_queueable_and_ignore_hidden_enemies`; `gpu::hud::tests::native_layout_hits_match_visible_controls_across_sizes_and_scales`; `gpu::hud::tests::resource_change_only_repaints_resource_plate_and_unchanged_layers_reuse` | Mechanism fixtures and CPU-composed UI images exist. They do not establish the native hit/input/render loop or real user readability. `frame_profile`/native screenshot hooks provide runtime timing only when executed on the PC. |
| GPU packets, display copies, kernels, rendering and explosion-only lighting | `shader_tests::gpu_kernels_validate`; `gpu::campaign_render_tests::display_records_validate_material_families_and_never_reduce_counts`; `gpu::campaign_render_tests::known_grown_materials_are_reused_and_invalid_completion_is_rejected` | WGSL is parsed/validated and CPU display records checked; no test here creates a GPU device. Older isolated GPU movement results are a separate workload. CPU ecology, water, tactical and construction ownership are not offloaded merely because display packets use wgpu. Explosion shadows, GPU barriers, uploads, frame stability and backend parity need actual native cases. |
| Append-only logging, metrics and frame-stage reporting | `metrics::tests::bounded_ring_reports_recent_quantiles_and_window_max_then_clears`; `metrics::tests::nearest_rank_quantiles_preserve_a_single_frame_spike`; `metrics::tests::invalid_durations_do_not_poison_the_report` | `console.txt`, per-case stdout/stderr and native stage reports are available. `logging.rs`, `diagnostics.rs`, `frame_profile.rs`, portions of `play.rs` and `river.rs` have no independent named unit family in the old inventory; their behavior is exercised by relevant reports/windows, not silently counted as additional tests. |
| Production map types/settings,20km streaming, sanctuary management, mutation, audio/tutorials and native updater | No exact available test implements these complete features. Help/bookmarks, procedural seeds, Pen, stored species masks and overlay updater scripts are neighboring implemented features. | **Planned/unimplemented** complete systems. Do not relabel existing Pens as sanctuaries, seeded profile generation as runtime genetic mutation, a launcher download as a native click-to-update flow, or a4.8km resident map as20km streaming. |

## Selected 30 additional trait mechanisms

These IDs are **additional** capabilities, not the mandatory biological schema. Five32-bit words store160 flags; numeric profiles/reserves remain separate. All30 have available shared cost/effect fixtures. The table identifies actual consequences and natural evidence boundaries rather than treating flag presence as a pass. The other130 IDs remain planned.

Natural evidence shorthand: `physical` means `final.physical.committed_traits[ID]`; `state` means actual serialized species/plant/cohort/clutch/ticket fields, not a new counter. The present report does not provide a dedicated successful-effect counter for the19 nonphysical traits. Proposed future sampling must record a real before/after transfer, affected identity, finite cost and consequence; it must not manufacture an event to complete this table.

| ID / mechanism | Exact available fixture(s) | Natural runtime observation / remaining boundary |
|---|---|---|
| 11 Tree climbing | `habitat::tests::canopy_connectivity_host_loss_and_cavities_have_real_fall_outcomes`<br>`ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass` | `not_reached`: no natural physical11 commit in the short report. Live aggregate canopy host, actual aperture/occupancy and paid ascent/descent exist; per-tree supports are not modeled. |
| 12 Burrowing | `habitat::tests::bent_burrow_preserves_centerline_aperture_spoil_and_cost`<br>`habitat::tests::branching_routes_and_real_exit_do_not_duplicate_occupants`<br>`ecosystem_physics::tests::bent_tunnel_spoil_is_paid_from_underground_and_never_lowers_the_surface`<br>`campaign_habitats::tests::local_committed_cut_reveals_only_its_real_room_without_new_animals_or_score` | `not_reached`:12 is active/queued, not naturally committed in the short report. Bent graph, finite underground substrate/spoil and local roof breach are fixture-tested. Underground flooding is an entrance-connected level hazard, not another fluid-volume owner. |
| 46 Trap construction | `habitat::tests::webs_have_no_fake_crater_pits_need_exact_backfill_and_prey_is_unique`<br>`ecosystem_physics::tests::fine_pit_uses_exact_remainder_owner_instead_of_free_or_lost_coarse_soil` | `observed` physical commit46=2. This proves paid trap construction, not a naturally observed unique prey interception/meal. Webs do not invent craters; pits debit exact soil. |
| 61 Seasonal migration | `ecology::tests::migration_moves_once_and_transit_seed_keeps_species_viable`<br>`ecology_navigation::tests::continuous_wall_and_later_opening_change_both_sides` | `not_reached` as a specifically attributed seasonal consequence. Transit stock is naturally present, but hunger/thirst/threat/pollen can also cause transit. Real travel costs/timing and masks exist; in-flight edge closure/internal border access need broader checks. |
| 64 Den dependence | `habitat::tests::den_reservation_requires_reachable_room_and_does_not_double_room_capacity`<br>`habitat::tests::engineering_tracks_actual_group_size_and_body_width_in_refuge_capacity`<br>`ecology::tests::aggregate_shelter_cannot_authorize_breeding_and_split_does_not_clone_claim`<br>`ecosystem_physics::tests::paid_nested_nest_and_exact_mother_claim_share_real_den_capacity` | `not_reached` natural64 commit/reproductive reservation. Wild breeding now uses the exact occupied accessible den claim; aggregate shelter_capacity does not authorize it. |
| 65 Nest construction | `ecosystem_physics::tests::real_nested_construction_transfers_finite_biomass_and_survives_json_save`<br>`habitat::tests::finite_den_capacity_and_nest_clutch_do_not_spawn_animals`<br>`habitat::tests::canopy_nests_flood_only_when_real_water_reaches_their_elevation`<br>`ecosystem_physics::tests::paid_nested_nest_and_exact_mother_claim_share_real_den_capacity` | `observed` physical commit65=5. Specific paid builder-owned nest and finite nested-room/egg slots are tested. The natural captive offspring gate proves Pen breeding, not every wild nest/host/flood outcome. |
| 67 Wallowing | `habitat::tests::repeated_wallowing_persists_and_dry_wallow_cannot_cool` | `not_reached` natural67 commit. Paid wet-site excavation persists; finite residual soil is retained. Full species-specific thermoregulation and every compaction/traction response are not established by the fixture name. |
| 72 Seasonal dormancy | `ecology::tests::dormancy_spends_reserves_and_flooding_removes_its_protection` | `not_reached` as an individually logged natural dormancy/wake sequence. State includes dormant_until and real cohort-owned refuge. Costs continue; flooding removes protection. Biological dry predicate versus named climate phase needs cross-season review. |
| 75 Dehydration tolerance | `ecology::tests::dehydration_tolerance_changes_damage_but_does_not_remove_water_need` | `not_reached` as a paired natural tolerance response. Finite hydration, continued demand and changed injury threshold exist; species-specific tradeoff/balance remains development calibration. |
| 84 Sperm storage | `ecology::tests::stored_sperm_is_finite_and_pregnancy_follows_capture_release_without_duplication`<br>`ecology::tests::materialization_preserves_finite_pollen_seeds_sperm_and_counts` | `not_reached` as a recorded natural stored-dose conception. SpermTicket doses/expiry and conception cost are real; no detailed donor genome or free self-restoration. |
| 86 Temperature-dependent sex determination | `ecology::tests::incubation_role_commits_once_and_guarding_has_a_finite_cost` | `not_reached` as a separately demonstrated temperature-caused natural role outcome. One thermal decision at hatch is tested; no separate developmental subwindow/pupal pipeline. |
| 93 Egg guarding | `ecology::tests::incubation_role_commits_once_and_guarding_has_a_finite_cost` | `not_reached` natural guarding-cost effect attribution. Real material reserve debit and existing clutch references exist; fine defensive combat/pursuit is not complete. |
| 96 Metamorphosis | `ecology::tests::metamorphosis_pays_body_growth_and_milk_moves_only_from_actual_parent` | `not_reached` natural96 stage-specific transition evidence. Species/stage-specific habitat/locomotion/diet and paid body growth are tested. Current stages are Juvenile/Adult, without a general pupa/subadult/senescent stage. |
| 101 Dam construction | `ecosystem_physics::tests::paid_animal_dam_changes_actual_water_links_without_creating_liters`<br>`ecosystem_physics::tests::animal_obstruction_layers_cannot_open_human_barrier_and_breach_removes_only_own_layer`<br>`campaign_habitats::tests::blast_breaches_only_near_dam_erodes_reef_and_preserves_water_and_material` | `not_reached` natural101 commit/reservoir cycle. Actual managed links, finite wood, leakage/breach and barrier ownership exist; dams elsewhere are rejected. Automatic overtopping wear and world-wide catchments remain incomplete. |
| 102 Soil mixing | `habitat::tests::soil_mixing_moves_nutrients_not_free_fertility_and_costs_work`<br>`ecosystem_physics::tests::soil_mixing_changes_actual_root_nutrient_access_and_conserves_both_owners` | `not_reached` natural102 commit. Actual two-layer material/nutrients and derived accessible-root/porosity coupling exist; no free fertility or duplicate soil owner. |
| 103 Tree thinning | `habitat::tests::thinning_moves_real_biomass_to_wood_and_rejects_overdraw`<br>`ecology::tests::canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources`<br>`ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass` | `not_reached` natural103 commit. Removed tissue becomes one real wood wallet; current roots/cover/support feedback is connected. Selection is aggregate patch vegetation, not a specifically selected individual tree. |
| 104 Cavity excavation | `habitat::tests::canopy_connectivity_host_loss_and_cavities_have_real_fall_outcomes`<br>`ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass` | `not_reached` natural104 commit. Finite host tissue/aperture/support and falls exist; aggregate hosts and incomplete general cavity-use AI limit fidelity. |
| 105 Reef building | `habitat::tests::dead_reef_builders_leave_physical_substrate_and_draft_limits`<br>`campaign_habitats::tests::blast_breaches_only_near_dam_erodes_reef_and_preserves_water_and_material`<br>`ecology_navigation::tests::water_crossings_require_connected_depth_and_respect_reefs` | `not_reached` natural105 commit. Finite mineral substrate persists after builder death and affects draft/clearance. Living-builder-count refresh and general aquatic refuge behavior need further coupling proof. |
| 106 Pollination | `ecology::tests::pollination_and_internal_seed_transport_have_finite_pollen_rewards_and_delay` | Actual pollen tickets are present on naturally captured species11 in the connected diagnostics: partial state evidence. Successful cross-patch service→plant reproduction is not separately quantified, so full natural consequence remains `not_reached` by this report. Finite pollen/nectar transfers are fixture-tested. |
| 107 Internal seed dispersal | `ecology::tests::pollination_and_internal_seed_transport_have_finite_pollen_rewards_and_delay`<br>`ecology::tests::materialization_preserves_finite_pollen_seeds_sperm_and_counts` | `not_reached` as a recorded natural delayed seed-deposition consequence. Real seeds move once through digestion tickets; no species-specific digestion loss model yet. |
| 121 Deep roots | `ecology::tests::deep_roots_access_only_finite_deep_store_and_flood_tolerance_is_limited` | Finite shallow/deep uptake participates in biology, but this trait’s natural differential is not separately quantified (`not_reached` attribution). Deep root water is a real finite ecological store, not a full spatial aquifer. |
| 125 Flood tolerance | `ecology::tests::deep_roots_access_only_finite_deep_store_and_flood_tolerance_is_limited` | Natural flood/death stocks exist, but tolerant versus susceptible exposure/outcome is not separated (`not_reached` attribution). Duration/depth response is a simplified profile, not calibrated root-oxygen physiology. |
| 128 Pioneer establishment | `ecology::tests::pioneer_seed_bank_resprouting_and_buoyancy_use_real_viable_stocks` | Living plant count changes naturally, but pioneer-specific germination versus other births is not attributed (`not_reached`). Real viable seed, finite nutrients/water and coarse disturbance/bare cover gate it. |
| 129 Root resprouting | `ecology::tests::pioneer_seed_bank_resprouting_and_buoyancy_use_real_viable_stocks` | `not_reached` as a natural identified root→regrowth event. Surviving root stock funds regrowth, without seed-based rescue. Repeated due steps provide a finite rate; no separate general regrowth timer. |
| 132 Persistent seed bank | `ecology::tests::pioneer_seed_bank_resprouting_and_buoyancy_use_real_viable_stocks`<br>`ecology::tests::extinction_never_recovers_from_generation_seed_without_surviving_stages` | Seed stock is naturally present; prolonged-bank survival→germination is not separately attributed (`not_reached`). Expiry/viability are real. Soil burial/scour/mixing do not yet transport banks with sediment. |
| 135 Animal-pollinated flora | `ecology::tests::pollination_and_internal_seed_transport_have_finite_pollen_rewards_and_delay` | `not_reached` as a naturally identified compatible pollen-service→finite seed output. Coarse patch/species compatibility is implemented; flower-level phenology is not. |
| 143 Buoyant seeds | `ecology::tests::buoyant_seeds_follow_actual_flux_fraction_and_preserve_fractional_credit_without_double_hops` | `not_reached` as a natural identified flow-driven seed relocation. Parent supplies real accumulated macro water flux and finite fraction/remainder; no motion occurs without flux. Transport chooses one macro destination, not particle fluid dynamics. |
| 156 Dense ground cover | `ecology::tests::canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources`<br>`ground::tests::current_root_and_cover_loss_changes_erosion_without_moving_stocks_on_assignment`<br>`ground::tests::live_cover_roots_and_canopy_change_actual_infiltration_and_evaporation_conservatively` | Current living cover projects into real ground conditions. Aggregate erosion/infiltration is observed; an individually attributed cover-loss→response comparison remains `not_reached` naturally. |
| 157 Broad shading canopy | `ecology::tests::canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources`<br>`ecology::tests::incubation_role_commits_once_and_guarding_has_a_finite_cost`<br>`ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass` | Current canopy changes biological temperature, evaporation/shade and support presence. Natural aggregate state is present, but a specific canopy-loss/fall/thermal consequence is `not_reached` in the short case. No expensive continuous rendered lighting or canopy rain interception is implied. |
| 159 Nutrient-rich litter | `ecology::tests::canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources` | `not_reached` as a natural identified litter-quality→decomposition differential. Finite litter/fungi/nutrient conversion exists. Quality is derived from living producer presence, rather than persistent composition per litter parcel; biological litter is not yet sediment-entrained. |

## Mandatory biology is additional to the 30

| Required family | Present finite mechanisms and exact check | Limits the soak must keep visible |
|---|---|---|
| Body/anatomy/mass/size/limbs/locomotion/habitat/tolerance | `Species` profile; actual radius/aperture/mass, habitat_for/locomotion_for; `ecology::tests::generated_food_web_and_anatomy_are_compatible_and_deterministic`; local physical route/fit fixtures | Fifteen parameterized archetype families, not free body-part evolution. The generation seed does not establish all intended anatomy/trait combinations. |
| Diet and compatible food web | Finite leaves/fruit/nectar/detritus/prey; no source duplication; generation, pollination and extinction fixtures | Explicit omnivory, preferred/fallback nutrition quality hierarchy, fine predator pursuit/defense and every species-specific digestion rule remain incomplete. A compatible food edge is weaker than long-term ecological viability. |
| Metabolism/hunger/hydration/health/stamina/age/development | Profile-scaled energy/water periods; real shortages/temperature/old-age injury/death; `ecology::tests::cross_seed_population_water_material_ledgers_and_serialized_continuity`; dehydration/metamorphosis fixtures | Normalized development units are not calibrated real-world physiology. Shared cohort age/condition deliberately aggregates offscreen members; independent materialized animals remain distinct. Multi-hour/multi-seed balance is not proven by exact stock conservation. |
| Day/night, slow movement, temperament/group/territory/threat | Rest gates feeding/engineering, slow travel changes real time; behavior helper cost/utility fixtures; real handler work | Binary day/night rather than arbitrary activity windows; coarse social utility rather than a full family/fear/defense AI. Extreme-input helper tests bound scores without clamping organism counts. |
| Reproductive roles, partners, maturity, intervals and seasons | Female/Male/Hermaphrodite schema, compatible same-pen roles or finite stored sperm, energy/water/age/health gates; `ecology::tests::concurrent_live_gestations_need_distinct_real_mothers_and_follow_materialization` | Wild reproduction uses season/refuge prerequisites. Captive breeding deliberately has no wild-season gate. No universal two-sex assumption in data, but generated diversity is limited; no detailed inherited genome. |
| Gestation/eggs/incubation/juveniles/parental care | Live gestation follows real mother custody; eggs stay at actual site; finite clutches, thermal sex assignment, guarding cost, exact nursery capacity, growth and real parental milk | Reproduction enum currently seeds/spores/eggs/live birth; retained eggs, spawning and rarer modes remain planned. Juvenile/Adult stages only; full carried-young/parental defense AI is not complete. |
| Milk and harvesting | Actual mother makes finite milk only for living dependent young; `ecology::tests::metamorphosis_pays_body_growth_and_milk_moves_only_from_actual_parent` | `Ecology::harvest_milk` exists and debits actual milk, but has no listed direct test or campaign/player-tool caller. Nursing tests do not prove a player milk-harvesting workflow. |
| Plant roots/shoots/leaves/output/age/needs/competition | Finite plant patches, moisture/nutrients/temperature/shade, uptake and output conversion; roots/flood/pioneer/canopy fixtures | Simplified nutrient/material growth rather than a complete carbon/oxygen/photosynthesis model. Fine pixel substrate, per-tree anatomy and full light-response curves are not simulated. |
| Litter/decomposition/fungi and recovery | Real litter→nutrient/respiration, fungal biomass, viable seed/root recovery; exact extinction fixture | Fungi are finite supporting biomass; microbes are not a separately parameterized full catalogue. Rich-litter persistence, sediment-carried banks and full aquifers remain limitations. |
| Exact offscreen ownership and permanent extinction | Cohort→individual→transit→capture→release transfer, tickets/pregnancy and ledgers; `ecology::tests::materialization_preserves_finite_pollen_seeds_sperm_and_counts`; `ecology::tests::materialized_capture_transport_housing_release_preserves_identity_and_finite_pregnancy` | There is no hidden rescue floor/spawn. A natural birth/germination must trace to living/viable capacity and real stock. Sparse materialization/claim behavior does not prove every future cohort-regrouping/genetic-variation feature. |
| Conservation infrastructure and scoring | Implemented names are **Pen**, **Research**, care, research stages, compatible breeding, and physical release; final small connected lifecycle gates passed | No distinct **Sanctuary** BuildKind, protected area/land-management/rewilding scoring system, complete zoo/veterinary model or complete causal ecological verdict is implemented. Do not use these names as synonyms for Pen or attribute natural deaths to factions without a causal owner. |
| Mutations and heritable variation | Seeded optional trait generation is present | Runtime mutation/genome inheritance machinery is planned. Report `unimplemented`, not zero successful mutations, until a real mechanism/event exists. |

## Windows portability audit

This is source inspection plus existing host `--list`, not Windows execution. No Wine, external environment installation, Cargo build or native source edit was used for this audit.

| Area | Finding | Packaged test-runner requirement |
|---|---|---|
| Test-name cfg differences | No Windows-only or Unix-only test-function/module gate was found in the inventoried snapshot. File-save and UI-recovery test families use `cfg(not(target_arch="wasm32"))`, so both ordinary host and Windows builds include them. Production internals still have Windows/Unix branches. New benchmark source must be re-audited at freeze. | Mark the expected catalog `inventory_source=host_cfg`; discover actual Windows names before execution and retain missing/additional names. Do not claim equality before the PC enumerates them. |
| Runtime source paths/assets | No `CARGO_MANIFEST_DIR`, `/workspace`, `/tmp` literal or absolute development-path runtime dependency was found in `native/src` tests. Terrain golden reference JSON, pixel atlas, fonts and WGSL use `include_str!`/`include_bytes!`. | Test executables do not require a Rust checkout or Python installation to read those fixtures. Keep binary hashes/source identity in the package catalog. |
| Optional external UI overrides | `hud::Theme::global` can read `ui/theme.json`; fonts can read optional `ui/` atlases/metrics before embedded fallback. This is a real working-directory dependency even though embedded assets exist. | Run mechanism tests in a fresh writable per-case directory with no inherited `ui/` customization. Record that embedded defaults were tested. Native game/UI capture cases separately exercise the shipped/user UI directory. Do not overwrite the user's layout/theme to make tests pass. |
| Test harness versus game main | Rust libtest replaces normal `main`; `last-world` test EXE does not execute the game's `current_exe` working-directory reset/logging bootstrap. The CLI tests call parser helpers, not the complete process option matrix. | Set working directory explicitly. `--list`/`--exact` must target the test harness, not the playable EXE. Complete CLI process/backends/window options need their separate cases. |
| Temporary files and Unicode paths | Save tests use `std::env::temp_dir()` plus process ID and atomic sequence; UI recovery adds timestamp; scenario save comparison uses a unique temp child. Cleanup is RAII/scoped, not deletion of user saves. Paths are `PathBuf`/`OsString`; Windows replacement converts to UTF-16. | Record inherited `TEMP`/`TMP`; give each child an explicit writable per-attempt temp subdirectory through its process environment, without changing the user environment. This also avoids stale PID/sequence-based paths after a crash/retry. Retain per-case files on failure if the supervisor captures them. Do not reuse fixture paths across parallel test processes or discard prior user saves. |
| Windows save durability/locking | Actual branches use `OpenOptionsExt::share_mode(0)` for cooperative save lock and `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` for replacement. Unix uses flock/directory sync. Windows directory sync is intentionally not asserted via the Unix branch. | Execute all four `campaign_support::tests::saves::*` plus UI corruption recovery on Windows. Host success cannot prove Windows share-mode/replace/durability or behavior under antivirus/file contention, power loss, network shares or disk full. |
| UI corrupt-file quarantine | Uses `create_new`, byte-for-byte copy, `sync_all`, explicit handle drop, then current-file removal; Unix-only directory sync is cfg-gated. Recovery test checks both originals survive next save. | Windows case must execute with actual file semantics. Retain quarantined originals/errors; skip only the failing case after bounded attempts, not all saves/tests. |
| Process/environment assumptions | No test starts a shell/Cargo/Python subprocess, sleeps for a timing assertion, changes environment variables or needs a network service. Runtime logging uses wall timestamps/process IDs; temporary names are not golden comparisons. The UI-recovery test uses an `UNIX_EPOCH` duration unwrap when naming its temp directory, so it assumes a normal post-1970 system clock; record clock failures rather than concealing that individual failure. Diagnostics report parallelism but tests do not assert a fixed core count. | No toolchain installation is needed on the PC. Record OS/process/environment/working-directory metadata. Each test gets a fresh process so statics, font caches and logs cannot contaminate another case. |
| GPU versus CPU | Every currently listed Rust test is a CPU mechanism/serialization/layout/shader-validation test. No listed test requests a wgpu adapter or opens a native window. | Label this stage CPU correctness. Actual Vulkan/DX12/native spectator/input/presentation cases remain separate and can fail while every Rust test passes. Inspect imports/dependencies of each newly shipped harness, not only the game EXE. |
| Heavy fixtures | Terrain/construction/physics tests allocate resident terrain; Campaign mortality/habitat/command fixtures still contain600 ecological patches and14,800 initial wildlife even with2 military/2 workers. Construction tests can run12,000 ticks; tactical scale test preserves10,000+2,000 agents but only20 steps. HUD layout-only coverage loops through4K sizes/scales; the cache-raster fixture draws1366×768, so the layout test is not a4K GPU load. Habitat replay is16×64 action attempts; ecology seed fixture is8×120 macro steps. | One exact case per process bounds cross-case heap retention and permits retry/timeout at case granularity. Use a generous per-case deadline and record elapsed/peak memory; a test taking longer on a low-end PC is not a reason to cut its counts/steps. Do not interpret this test duration as game FPS. |
| Fatal exits/crashes | A panic usually exits101; OS access violations, OOM or missing DLLs can return other/nonzero codes or terminate before libtest's summary. A nonexistent exact name can exit0 while running0 tests. | Require exit0 **and exactly one executed passing case**, with0 failed/ignored; retain stdout/stderr/status. Detect startup failures and timeouts explicitly. Never let an empty filter become a pass. |

No concrete native portability bug requiring a source edit was found in this audit. Windows runtime remains unproven until its actual discovery and case executions return. The existing two float-coordinate failures are semantic regression history, not erased by this audit; their raw logs remain alongside the passing batch.

## Builder discovery and shipped harness contract

Commands below describe the builder/packager, not a command the player must install Cargo to run. The parent owns build/stamp/packaging implementation. Use the frozen source/vendor/assets identity and exact resulting EXE hashes.

```powershell
# Builder only; capture Cargo exit status and JSON lines.
cargo test --manifest-path native/Cargo.toml --locked --release `
  --target x86_64-pc-windows-gnu --no-run --message-format=json
```

Select `compiler-artifact` messages with `profile.test == true`, a non-null `executable`, the expected package ID, and the expected logical target/source:

- library target name `last_world_native`, source basename `lib.rs`;
- CLI target name `last-world`, source basename `main.rs`.

Do not choose an artifact by newest timestamp, guessed hash filename, substring `last_world`, or `target.kind == ["lib"]`: this manifest declares `cdylib` and `rlib` crate types, so a robust selector uses logical target identity plus test profile and actual executable. Reject zero/multiple ambiguous selections. Retain JSON metadata, Cargo success, compiler/target/profile, frozen source identity and binary SHA256. `fresh=true` cached artifacts still need matching frozen content stamps.

An equivalent selection sketch is:

```powershell
$artifacts = $jsonObjects | Where-Object {
  $_.reason -eq 'compiler-artifact' -and
  $_.profile.test -eq $true -and $_.executable -and
  $_.package_id -eq $expectedPackageId
}
$library = @($artifacts | Where-Object {
  $_.target.name -eq 'last_world_native' -and
  [IO.Path]::GetFileName($_.target.src_path) -eq 'lib.rs'
})
$cli = @($artifacts | Where-Object {
  $_.target.name -eq 'last-world' -and
  [IO.Path]::GetFileName($_.target.src_path) -eq 'main.rs'
})
if ($library.Count -ne 1 -or $cli.Count -ne 1) { throw 'Ambiguous test harness artifacts' }
```

Generate the expected name list from the **new frozen host harness built from the same sources**, identifying its host cfg. Windows `.exe` files cannot be executed with `--list` on this Linux host. On the PC, enumerate each actual shipped harness first:

```powershell
& $libraryTestExe --list
& $cliTestExe --list
# Then a separate child process for EACH discovered exact test name:
& $libraryTestExe --exact 'water::tests::dam_fills_gradually_throttles_downstream_and_inundates_connected_bank' --test-threads=1 --nocapture
```

Parse only lines ending `: test`; the totals line and benchmark labels are not cases. Store the actual Windows set and expected/actual differences before running tests. Hash both harnesses and the discovered catalog. Use argument arrays/ProcessStartInfo, not a reconstructed shell command. Each case runs in its own writable directory; capture both streams, elapsed/peak memory, timeout, exit code and actual libtest counts.

The current supervisor uses **at most two total attempts per exact phase**: the initial attempt plus one retry, then a failed/skipped-case record and continuation with the next case. The CMD watchdog separately permits two supervisor restarts, which resume the durable journal rather than resetting per-case attempt budgets or rerunning completed phases. Retain every failed/interrupted attempt even if a retry passes. “Skip this failing case and continue” cannot produce an all-passed summary; report complete/pass/fail/timeout/skipped totals separately. Do not retry the entire217-case suite, discard previous failure evidence, silently shorten heavy fixtures, or infer pass from child exit0 alone. The actual refreshed catalog may contain more than217 names.

The package runs prebuilt native harnesses and the game/spectator binaries. The player installs no Rust, Cargo, Python, Wine or dependencies from a package manager. Required OS/runtime DLLs still need PE import inspection and real startup validation; compiling a Windows EXE is not sufficient.

## Long-soak and cross-feature coverage gaps

Existing seed coverage is deliberately bounded:

| Existing evidence | What it actually covers | What it does not cover |
|---|---|---|
| Ecology seeds0..7,12patches,20..2400ticks plus next2420 after JSON roundtrip | Exact finite organism/material/water ownership, static wet environment and continuity across eight seeded catalogues | One full72,000-tick season, actual600-patch geography, live river/armies, every trait combination or emergent balance |
| Habitat seeds1..16,64 generated action attempts each | Ten physical generic actions (102 mixing has a separate real-layer fixture), failed-action atomicity, sparse graph validity and save roundtrip | Every attempted action succeeding, natural material collection/AI scheduling, every topology/support/flood interaction |
| Small connected seed78421 through17,320 plus78422/78423 at200ticks each | Actual paid construction/mining/combat and Pen lifecycle, early/late full saves, two short cross-seed invariant smoke checks | Full population, seasonal ecology, mature war, all30 natural traits, low-end hardware or every map type |
| Actual600 starting habitat, isolated72,000-tick ecology | One coherent seeded food web surviving a full cycle with finite ledgers and no founder cuts/rescue | Evolving full-world hydraulics/nav, warfare, rendering, every seed, multi-cycle equilibrium or production scale |
|10,000 military+2,000 worker tactical fixture,20ticks | Exact count ownership, initial formation/crowd scratch and20 tactical updates | Full ecology/construction/GUI workload for hours, frame pacing or achieved4×/100× |

The new native spectator/soak driver source now exposes top-level fields `case_id`, `process_id`, `session_id`, `started_utc_ms`, `status`, `passed`, `requested_rate`, `actual_counts`, `metrics`, `coverage`, `checkpoint`, `source_identity`. These are **new driver-source fields awaiting its new compiled/native qualification**, not retroactive fields in the old connected report. Its coverage entries use `status: observed|not_reached|unimplemented` plus actual `evidence` for `military`, `construction`, `logistics_economy`, `conservation`, `water_ground`, `physical_ecology`, `recon`, `seasons`, `saves`, `mutation`, `traits`, and `order_exercise`.

Per the driver owner, `physical_ecology.evidence` will retain initial/final reports and real per-trait commit deltas from `physical.report().committed_traits` / `ecology.physical_effects_committed`. These cover the11 physical mechanisms, not all19 additional biological ones. `traits` records the actual selected catalogue union/count, not an assertion that every mechanism ran. Mutation remains `unimplemented` without a real event mechanism.

The baseline soak should process at least72,000 additional **ordinary connected ticks** with unchanged10,000 military/2,000 worker requests and all naturally owned wildlife. Short seed78422/78423 cases of30 wall seconds are smoke tests, even if a requested diagnostic multiplier is high. If a war ends, a test times out, a system is unsupported, or rules never reach a trait, state that exact limit rather than forcing a capability/spawn/score to make coverage green.

Record wall elapsed, actual ticks/sim seconds, achieved/requested ratio, pending/max debt and actual calls/batches. A multi-tick `advance()` call duration quantile is a **batch-duration** metric; it is not a per-tick or presented-frame p95. Diagnostic demand rates1/2/4/8/16/32/64/100 must retain all debt and preserve normal playable1/2/4 options; a high requested rate never proves a high achieved rate. Native renderer frame intervals, GPU stage timing and memory need their own measurements.

Scheduler ceiling must also be explicit. Ordinary gameplay retains its16tick advance budget; the **benchmark-only profiled advance permits64ticks per call**. At one call per60Hz rendered frame, the benchmark policy has a theoretical192× simulation-rate ceiling (`64 × 60 / 20`), rather than the earlier16tick policy's48× ceiling. This is a scheduler arithmetic limit, never a hardware or achieved100× claim. Report `tick_budget_per_advance_call`, actual `advance_calls_per_wall_second` and `scheduler_ceiling_simulation_rate` separately from achieved throughput. Actual call rate, work, presentation or another scheduling policy can lower/change that ceiling.

The new `advance_benchmark_profiled` API returns a bounded stack array of64 **actual individual fixed20Hz step durations**. The driver exposes `metrics.actual_fixed_20hz_tick.{p50_ms,p95_ms,p99_ms,max_ms,count,distribution_complete,quantile_scope}`, `advance_batches`, `complete_frame_intervals`, and `pending_ticks_start/end/max`. This allows genuine per-step quantiles for its recorded samples, separately from batch/frame measurements; report distribution completeness and scope rather than assuming every sample is retained. The old connected report has no retroactive per-step profiling, and the new source still needs its compiled/native benchmark results.

Prioritize these still-missing combined sequences rather than increasing nominal test-count claims:

- Multiple seeds through several complete seasons and the intended3–4hour war; natural food/water search, partner availability, predation and finite source balance, without rescue floors or founder reductions.
- Water rise/scour/air exposure/drying + plant/root loss/regrowth + mud-road travel + actual migration-edge closure, with named cross-owner transfers and before/after stock identities.
- Dam building/leakage/breach, riverbank inundation and downstream supply, followed by actual breeding/foraging/capture or supply effects; keep unsupported unmanaged-water locations `not_reached`/`unimplemented`.
- Burrow branches/body fit/blocked entrances/flood/roof cuts + actual occupants/offspring; canopy thinning/host loss + exact cavity/nest/animal fall outcomes; den/nest claims never duplicated by splitting or save/load.
- All30 selected natural consequences, with effect/cost/identity evidence. Do not count pending requests as commits, physical commits as proof of all biology, pollen carriage as complete pollinated reproduction, or trait union as usage.
- Existing-animal capture, in-transit pregnancy, pen capacity/destruction, handler death, natural captive death, separate-pen incompatibility and exact release after late save/load. Pens must remain distinct from future sanctuary gameplay.
- Long-lived artillery/industry/ecological-damage attribution, downstream causality and the actual lower-damage stalemate verdict; no complete verdict test currently establishes that whole loop.
- Reefs/bridges/docks/transport draft, actual boarding/unloading/sinking and island logistics, with no duplicate passengers or ghost crossings.
- Camera-trooper depletion/recharge, changing local occluders, radar stale anonymous echoes, offset satellite prewarm/reveal and renderer reentry at high map visibility; observer diagnostics must never leak into player intelligence.
- Mature saves after widespread edits and multiple cycles, multiple interrupted/corrupt/recovery sequences, Windows file contention, actual size/load time and continued next-tick equality. Preserve originals and failed attempts.

## Exact available test-name appendix

The following inventory is the old compiled host snapshot,214library +3CLI tests. It is deliberately retained for traceability. It is not an immutable package manifest and must not prevent new cases from being discovered. Each code-block line is a separate exact libtest name.

| Library namespace | Listed cases |
|---|---:|
| `campaign` | 2 |
| `campaign_commands` | 3 |
| `campaign_habitats` | 2 |
| `campaign_support` | 12 |
| `climate` | 3 |
| `clock` | 6 |
| `construction` | 11 |
| `ecology` | 32 |
| `ecology_behavior` | 5 |
| `ecology_navigation` | 2 |
| `ecosystem_physics` | 15 |
| `faction_support` | 8 |
| `gpu` | 5 |
| `ground` | 20 |
| `habitat` | 20 |
| `metrics` | 3 |
| `runtime` | 2 |
| `shader_tests` | 1 |
| `tactics` | 29 |
| `terrain` | 10 |
| `ui_interaction` | 11 |
| `water` | 12 |

### Library: campaign

```text
campaign::custody_mortality_tests::actual_captive_mortality_cancels_return_and_both_release_phases_without_respawn
campaign::custody_mortality_tests::killed_handler_releases_same_animal_wallet_and_detail_at_actual_carrier_position
```

### Library: campaign_commands

```text
campaign_commands::tests::explicit_subset_moves_without_ordering_other_units_and_enemy_selection_is_atomic
campaign_commands::tests::queue_completion_requires_actual_arrival_and_replacement_affects_only_selected_ids
campaign_commands::tests::shift_queue_and_hold_are_owned_saved_and_bounded_without_changing_current_goal
```

### Library: campaign_habitats

```text
campaign_habitats::tests::blast_breaches_only_near_dam_erodes_reef_and_preserves_water_and_material
campaign_habitats::tests::local_committed_cut_reveals_only_its_real_room_without_new_animals_or_score
```

### Library: campaign_support

```text
campaign_support::tests::battery_runs_out_recharges_at_real_base_and_duplicate_tick_cannot_charge_it_twice
campaign_support::tests::circular_satellite_prewarm_precedes_visibility_and_team_passes_are_offset
campaign_support::tests::newly_spawned_camera_is_not_charged_for_time_before_it_existed
campaign_support::tests::ordinary_units_give_no_vision_and_saved_exploration_is_not_current_vision
campaign_support::tests::radar_echo_is_a_snapshot_not_hidden_live_tracking_and_expires
campaign_support::tests::recon_save_roundtrip_preserves_batteries_explored_and_unidentified_echoes
campaign_support::tests::remote_terrain_edits_preserve_local_los_cache_but_local_edits_invalidate_it
campaign_support::tests::saves::atomic_save_preserves_previous_and_ignores_interrupted_pending_files
campaign_support::tests::saves::corrupt_current_requires_explicit_recovery_and_cannot_poison_previous
campaign_support::tests::saves::noninteger_state_checksum_uses_saved_bytes_without_float_reformatting
campaign_support::tests::saves::unknown_schema_and_invalid_v1_migration_leave_original_untouched
campaign_support::tests::taller_solid_blocks_radar_and_camera_but_short_or_nonsolid_objects_do_not
```

### Library: climate

```text
climate::tests::living_jungle_has_real_wet_dry_forcing_without_universal_snow
climate::tests::save_value_roundtrip_and_max_clock_do_not_overflow
climate::tests::whole_year_is_integer_clock_deterministic_and_repeats_conditions
```

### Library: clock

```text
clock::tests::bounded_work_retains_all_debt_and_pause_neither_accrues_nor_drains_it
clock::tests::overflow_is_explicit_and_transactional
clock::tests::partitioning_wall_time_does_not_change_ticks_at_any_allowed_speed
clock::tests::season_cycle_deadline_is_exact_and_scales_with_selected_speed
clock::tests::speed_changes_do_not_reprice_previous_debt_or_fractional_time
clock::tests::unsupported_speeds_are_rejected_without_substitution
```

### Library: construction

```text
construction::tests::cancellation_at_site_needs_physical_material_recovery_before_refund
construction::tests::completed_solid_mine_loads_at_exterior_and_worker_hauls_real_ore_to_base
construction::tests::exhausted_building_identity_does_not_partially_clear_or_consume
construction::tests::invalid_orders_and_duplicate_workers_cannot_spend_or_duplicate_labor
construction::tests::jobs_require_real_pickup_delivery_and_work_then_consume_once
construction::tests::managed_trench_preserves_water_and_soil_transactions_survive_ticks
construction::tests::one_worker_walks_out_after_a_solid_build_and_completes_a_second
construction::tests::reservation_cancel_and_save_roundtrip_preserve_exact_resources
construction::tests::suspended_hauler_keeps_physical_load_and_job_then_resumes_delivery
construction::tests::trench_displaces_real_spoil_and_stale_jobs_do_not_overwrite
construction::tests::two_thousand_workers_keep_ids_and_only_assigned_reachable_workers_act
```

### Library: ecology

```text
ecology::behavior::tests::docile_handling_pays_real_time_but_less_than_identical_wild_animal
ecology::behavior::tests::extreme_inputs_bound_utilities_and_never_require_population_floors
ecology::behavior::tests::observed_threats_change_response_without_making_defensive_animals_immune
ecology::behavior::tests::projected_group_size_and_territory_change_destination_ranking
ecology::behavior::tests::regional_founders_are_not_treated_as_one_oversized_herd
ecology::tests::a_completed_pen_does_not_give_eggs_flood_immunity
ecology::tests::aggregate_shelter_cannot_authorize_breeding_and_split_does_not_clone_claim
ecology::tests::buoyant_seeds_follow_actual_flux_fraction_and_preserve_fractional_credit_without_double_hops
ecology::tests::canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources
ecology::tests::capture_care_research_breeding_release_are_real_finite_transfers
ecology::tests::concurrent_live_gestations_need_distinct_real_mothers_and_follow_materialization
ecology::tests::cross_seed_population_water_material_ledgers_and_serialized_continuity
ecology::tests::deep_roots_access_only_finite_deep_store_and_flood_tolerance_is_limited
ecology::tests::dehydration_tolerance_changes_damage_but_does_not_remove_water_need
ecology::tests::dormancy_spends_reserves_and_flooding_removes_its_protection
ecology::tests::extinction_never_recovers_from_generation_seed_without_surviving_stages
ecology::tests::founding_habitat_acceptance_relocates_only_existing_counts_once
ecology::tests::generated_food_web_and_anatomy_are_compatible_and_deterministic
ecology::tests::hidden_burrow_animals_cannot_be_discovered_captured_or_eaten_through_ground
ecology::tests::incubation_role_commits_once_and_guarding_has_a_finite_cost
ecology::tests::malformed_saved_references_and_stock_overflow_return_errors_instead_of_panics
ecology::tests::materialization_preserves_finite_pollen_seeds_sperm_and_counts
ecology::tests::materialized_capture_transport_housing_release_preserves_identity_and_finite_pregnancy
ecology::tests::metamorphosis_pays_body_growth_and_milk_moves_only_from_actual_parent
ecology::tests::migration_moves_once_and_transit_seed_keeps_species_viable
ecology::tests::physical_pen_delivery_capacity_reserves_offspring_and_loss_stops_care_without_deleting_animals
ecology::tests::pioneer_seed_bank_resprouting_and_buoyancy_use_real_viable_stocks
ecology::tests::pollination_and_internal_seed_transport_have_finite_pollen_rewards_and_delay
ecology::tests::separate_pens_do_not_allow_remote_breeding
ecology::tests::stale_due_ticks_rejected_without_mutation
ecology::tests::stored_sperm_is_finite_and_pregnancy_follows_capture_release_without_duplication
ecology::tests::thirst_search_respects_actual_border_masks_and_never_drinks_remote_stock
```

### Library: ecology_behavior

```text
ecology_behavior::tests::docile_handling_pays_real_time_but_less_than_identical_wild_animal
ecology_behavior::tests::extreme_inputs_bound_utilities_and_never_require_population_floors
ecology_behavior::tests::observed_threats_change_response_without_making_defensive_animals_immune
ecology_behavior::tests::projected_group_size_and_territory_change_destination_ranking
ecology_behavior::tests::regional_founders_are_not_treated_as_one_oversized_herd
```

### Library: ecology_navigation

```text
ecology_navigation::tests::continuous_wall_and_later_opening_change_both_sides
ecology_navigation::tests::water_crossings_require_connected_depth_and_respect_reefs
```

### Library: ecosystem_physics

```text
ecosystem_physics::tests::actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass
ecosystem_physics::tests::animal_obstruction_layers_cannot_open_human_barrier_and_breach_removes_only_own_layer
ecosystem_physics::tests::bent_tunnel_spoil_is_paid_from_underground_and_never_lowers_the_surface
ecosystem_physics::tests::bounded_local_route_does_not_cross_solid_walls_or_disconnected_ridges
ecosystem_physics::tests::coarse_materialization_uses_connected_real_habitat_and_known_animal_never_teleports
ecosystem_physics::tests::fine_pit_uses_exact_remainder_owner_instead_of_free_or_lost_coarse_soil
ecosystem_physics::tests::observed_builder_walks_over_multiple_updates_without_teleport_or_free_work
ecosystem_physics::tests::paid_animal_dam_changes_actual_water_links_without_creating_liters
ecosystem_physics::tests::paid_nested_nest_and_exact_mother_claim_share_real_den_capacity
ecosystem_physics::tests::queued_builder_gets_no_free_work_and_only_real_elapsed_awake_ticks_fund_tunnel
ecosystem_physics::tests::real_nested_construction_transfers_finite_biomass_and_survives_json_save
ecosystem_physics::tests::same_cell_final_approach_moves_and_pays_time_across_water_changes_and_save
ecosystem_physics::tests::soil_mixing_changes_actual_root_nutrient_access_and_conserves_both_owners
ecosystem_physics::tests::stalled_engineers_release_slots_with_saved_cooldowns_and_keep_wallets_and_identity
ecosystem_physics::tests::verified_custody_handoff_preserves_wallet_and_counts_cancels_old_jobs_and_saves_exact_xy
```

### Library: faction_support

```text
faction_support::tests::blocked_line_spends_nothing_and_opening_wall_allows_fire
faction_support::tests::exhausted_turret_does_not_receive_invisible_replacement_rounds
faction_support::tests::own_building_footprint_does_not_block_but_nearby_ridge_still_does
faction_support::tests::remaining_stock_is_lost_once_on_destruction_and_cannot_respawn
faction_support::tests::save_reload_keeps_cooldown_target_ammo_and_exact_continuation
faction_support::tests::supply_requires_local_physical_arrival_and_caps_actual_acceptance
faction_support::tests::turret_hits_real_target_on_cooldown_with_finite_paid_rounds
faction_support::tests::unsupported_or_unpaid_sources_and_reused_ids_are_rejected
```

### Library: gpu

```text
gpu::campaign_render_tests::display_records_validate_material_families_and_never_reduce_counts
gpu::campaign_render_tests::known_grown_materials_are_reused_and_invalid_completion_is_rejected
gpu::hud::tests::licensed_fonts_are_proportional_with_tabular_numbers_and_real_coverage
gpu::hud::tests::native_layout_hits_match_visible_controls_across_sizes_and_scales
gpu::hud::tests::resource_change_only_repaints_resource_plate_and_unchanged_layers_reuse
```

### Library: ground

```text
ground::tests::construction_cut_fill_transfers_exact_soil_without_moving_water
ground::tests::current_root_and_cover_loss_changes_erosion_without_moving_stocks_on_assignment
ground::tests::disconnected_material_does_not_cross_a_sealed_ridge
ground::tests::dry_deposition_raises_the_bed_using_existing_sediment
ground::tests::dry_submillimeter_sediment_residue_becomes_real_bed_stock
ground::tests::erosion_and_washout_change_depth_without_creating_soil_or_biomass
ground::tests::excessive_suspension_settles_at_a_bounded_rate_even_in_fast_water
ground::tests::explicit_foliage_exchange_requires_available_units_and_keeps_owners_exact
ground::tests::fractional_climate_rates_keep_accumulation_when_weather_changes
ground::tests::invalid_biomass_part_of_construction_rolls_back_all_staged_soil
ground::tests::invalid_construction_transaction_commits_no_partial_cell
ground::tests::invalid_tick_and_reused_water_flow_do_not_commit_ground_or_exchange
ground::tests::live_cover_roots_and_canopy_change_actual_infiltration_and_evaporation_conservatively
ground::tests::low_water_caps_new_erosion_and_closing_flow_stops_further_scouring
ground::tests::material_snapshot_moves_only_one_connection_per_tick
ground::tests::outlet_material_leaves_through_a_named_sink_and_remains_in_the_ledger
ground::tests::saturated_soil_with_submillimeter_film_still_gets_air_and_can_dry
ground::tests::shallow_flow_film_does_not_scour_soil_or_detach_foliage
ground::tests::soil_keeps_moisture_during_air_delay_then_dries_and_rewetting_resets_it
ground::tests::wet_settling_can_deposit_less_than_one_geometry_millimeter
```

### Library: habitat

```text
habitat::tests::bent_burrow_preserves_centerline_aperture_spoil_and_cost
habitat::tests::blocked_passages_flood_only_the_open_connected_graph
habitat::tests::branching_routes_and_real_exit_do_not_duplicate_occupants
habitat::tests::canopy_connectivity_host_loss_and_cavities_have_real_fall_outcomes
habitat::tests::canopy_nests_flood_only_when_real_water_reaches_their_elevation
habitat::tests::dam_damage_leaks_before_breach_and_repair_replaces_actual_wood_without_mass_loss
habitat::tests::dams_spend_finite_material_gradually_reduce_flow_and_breach_to_debris
habitat::tests::dead_reef_builders_leave_physical_substrate_and_draft_limits
habitat::tests::den_reservation_requires_reachable_room_and_does_not_double_room_capacity
habitat::tests::engineering_replay_across_seeds_preserves_invariants_and_failed_actions_are_atomic
habitat::tests::engineering_tracks_actual_group_size_and_body_width_in_refuge_capacity
habitat::tests::failed_cost_is_atomic_and_save_restores_real_graph_and_reservations
habitat::tests::finite_den_capacity_and_nest_clutch_do_not_spawn_animals
habitat::tests::local_crater_does_not_collapse_remote_burrow_in_same_macro_patch
habitat::tests::prepared_engineering_rejects_stale_jobs_and_never_creates_free_structures
habitat::tests::repeated_wallowing_persists_and_dry_wallow_cannot_cool
habitat::tests::soil_mixing_moves_nutrients_not_free_fertility_and_costs_work
habitat::tests::surface_cut_collapses_roof_and_exposes_actual_occupant_once
habitat::tests::thinning_moves_real_biomass_to_wood_and_rejects_overdraw
habitat::tests::webs_have_no_fake_crater_pits_need_exact_backfill_and_prey_is_unique
```

### Library: metrics

```text
metrics::tests::bounded_ring_reports_recent_quantiles_and_window_max_then_clears
metrics::tests::invalid_durations_do_not_poison_the_report
metrics::tests::nearest_rank_quantiles_preserve_a_single_frame_spike
```

### Library: runtime

```text
runtime::clock_integration_tests::control_consumes_elapsed_once_and_drains_only_remaining_tick_debt
runtime::clock_integration_tests::four_x_control_retains_unclamped_time_and_pause_preserves_owed_ticks
```

### Library: shader_tests

```text
shader_tests::gpu_kernels_validate
```

### Library: tactics

```text
tactics::tests::a_new_road_eventually_changes_preference_without_discarding_live_field
tactics::tests::boarding_and_release_keep_every_identity_owned_once_through_save
tactics::tests::bridge_destruction_removes_ghost_crossing_without_unchanged_overlay_churn
tactics::tests::changing_costs_do_not_starve_search_or_interrupt_live_cost_refresh
tactics::tests::combat_commits_real_casualties_and_conserves_ammunition
tactics::tests::cost_only_updates_preserve_cached_topology_and_idempotent_assignments
tactics::tests::cost_refresh_frontier_and_deadline_survive_save_exactly
tactics::tests::crowd_does_not_compress_two_walkers_into_same_goal
tactics::tests::discrete_route_budget_is_bounded_and_dead_pawns_do_not_work
tactics::tests::exact_formation_spawn_uses_only_free_passable_sites
tactics::tests::flood_evacuation_and_drowning_are_real_saved_finite_state
tactics::tests::foot_paths_do_not_treat_roads_as_bridges_and_naval_checks_clearance
tactics::tests::grid_orders_resolve_reachable_banks_without_teleporting_or_faking_work_reach
tactics::tests::harmless_water_changes_do_not_starve_a_partially_built_route
tactics::tests::idle_friend_yields_without_ghosting_enemies_or_other_workers
tactics::tests::more_distinct_jobs_than_cache_slots_are_eventually_serviced
tactics::tests::ordinary_flood_water_below_breathing_height_does_not_drown_stationary_troops
tactics::tests::partial_navigation_jobs_survive_save_without_changing_next_ticks
tactics::tests::reef_overlay_changes_clearance_once_without_depth_restore_churn
tactics::tests::retreat_and_new_solid_obstruction_do_not_leave_soldiers_frozen_on_old_targets
tactics::tests::shared_corridor_keeps_six_haulers_moving_through_a_dense_worker_spawn
tactics::tests::shared_route_has_one_build_and_flood_invalidates_it
tactics::tests::sinking_transport_and_repeat_damage_do_not_duplicate_passenger_deaths
tactics::tests::soil_moisture_updates_only_revise_route_cost_when_a_traction_band_changes
tactics::tests::ten_thousand_military_plus_two_thousand_workers_keep_exact_counts
tactics::tests::terrain_derived_wet_jungle_corridor_keeps_physical_haulers_progressing
tactics::tests::topology_masks_distinguish_cost_changes_and_foot_from_naval_clearance
tactics::tests::wet_soil_reduces_offroad_traction_while_roads_limit_the_mud_cost
tactics::tests::worker_labor_requires_real_arrival_and_duplicate_ticks_fail
```

### Library: terrain

```text
terrain::tests::dirty_ranges_are_deduplicated_disjoint_and_never_cross_rows
terrain::tests::drained_surface_keeps_wet_brown_state_until_owner_changes_moisture
terrain::tests::dynamic_water_repaints_substrate_once_and_keeps_texture_updates_local
terrain::tests::effective_height_and_slope_include_committed_cut_fill
terrain::tests::ground_cache_rejects_regrowth_and_invalid_state_without_committing
terrain::tests::matches_long_front_reference
terrain::tests::radial_cut_is_local_and_an_invalid_edit_changes_no_elevation
terrain::tests::removing_foliage_clears_existing_grown_pixels_locally_to_brown_soil
terrain::tests::signed_elevation_edits_report_exact_coarse_volume_and_reject_invalid_targets
terrain::tests::smoothing_reads_the_original_neighborhood_before_committing
```

### Library: ui_interaction

```text
ui_interaction::tests::alerts_merge_prioritize_and_jump_only_to_explicit_known_location
ui_interaction::tests::box_select_and_double_click_only_choose_visible_friendly_units
ui_interaction::tests::camera_bookmarks_and_foodweb_gestures_never_issue_world_orders
ui_interaction::tests::contextual_orders_are_selected_only_queueable_and_ignore_hidden_enemies
ui_interaction::tests::control_groups_recall_known_refs_without_learning_hidden_live_positions
ui_interaction::tests::corrupt_layout_recovers_previous_and_next_save_preserves_both_originals
ui_interaction::tests::hud_clicks_never_fall_through_to_world_and_placement_cancels_cleanly
ui_interaction::tests::invalid_layout_is_rejected_and_edgescroll_is_optional
ui_interaction::tests::minimap_navigation_zoom_and_resizing_change_only_ui_or_camera_command
ui_interaction::tests::panel_layout_and_accessibility_preferences_roundtrip_without_gameplay_state
ui_interaction::tests::rally_board_and_disembark_tools_never_issue_legacy_global_actions
```

### Library: water

```text
water::exchange_tests::cross_owner_exchange_is_exact_and_failing_batch_is_atomic
water::exchange_tests::exposed_transport_snapshot_is_before_transfer_and_does_not_change_on_exchange
water::tests::breach_opens_flow_but_does_not_teleport_or_delete_reservoir
water::tests::dam_fills_gradually_throttles_downstream_and_inundates_connected_bank
water::tests::disconnected_low_ground_does_not_fill_through_a_high_ridge
water::tests::equal_water_heads_do_not_create_circulation
water::tests::finite_rates_and_transfers_preserve_exact_water_balance
water::tests::high_crest_does_not_prevent_a_lower_open_connection_from_draining
water::tests::invalid_settings_and_overflow_are_explicit_and_do_not_commit
water::tests::multiple_sealed_outlets_cannot_jointly_drain_below_their_crest
water::tests::simultaneous_donors_cannot_spend_the_same_volume_twice
water::tests::snapshots_prevent_double_distance_from_link_iteration_order
```

### CLI test harness

```text
tests::counts_reject_invalid_input_without_reducing_requested_workload
tests::duration_rejects_nonfinite_and_nonpositive_values
tests::speed_accepts_only_explicit_testing_options
```

### New source cases not in the old compiled list

At audit time `clock.rs` additionally contains `clock::tests::benchmark_hundred_rate_keeps_exact_debt_and_does_not_unlock_playable_speed` and `clock::tests::benchmark_rate_changes_preserve_previously_charged_time_and_fraction`. They test diagnostic clock arithmetic, not100× throughput. More driver/packaging tests may land before freeze. The new host/Windows harness inventories, their source identities and actual case results supersede the old appendix for execution.


To honor the single-speed-test policy, all other focused smoke/population/seed/recovery probes request only1x; the72,000-tick seasonal soak requests2x and may take roughly30minutes or longer if achieved throughput falls. These phases test load/correctness, not successive speed ceilings. Only the adaptive ramp can ask for4/8/16/32/64/100x. Actual debt and achieved timing remain reported for every phase.
