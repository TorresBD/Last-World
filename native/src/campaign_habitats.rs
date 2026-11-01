//! Campaign handoff for local terrain edits and destructible ecosystem structures.
//!
//! Terrain/ground already own the excavation transaction. These helpers apply
//! its habitat consequences once; they never excavate a second soil volume or
//! charge the faction ecological-damage score. Water remains in WaterGraph.

use crate::{
    campaign::{Campaign, patch_at},
    habitat::{HabitatEvent, Occupancy},
    terrain::{CELL_METERS, CH, CW},
};
use std::collections::BTreeMap;

impl Campaign {
    /// Reconcile already committed fine-cell heights with only intersecting
    /// underground roofs. A cut in one8m cell cannot expose an entire160m patch.
    /// Inputs are final authoritative heights, not requested height deltas.
    pub fn reconcile_surface_habitat_edits(
        &mut self,
        edits: &[(usize, i32)],
    ) -> Result<(), String> {
        let mut unique = BTreeMap::new();
        for &(cell, height) in edits {
            if cell >= CW * CH {
                return Err("Habitat terrain edit outside resident world".into());
            }
            if self.terrain.elevation_mm(cell).map_err(|e| e.to_string())? != height {
                return Err("Habitat terrain edit does not match committed height".into());
            }
            if unique.insert(cell, height).is_some_and(|old| old != height) {
                return Err("Conflicting habitat terrain edit heights".into());
            }
        }
        if unique.is_empty() {
            return Ok(());
        }

        // Preserve the actual room position before collapse releases its
        // occupancy reference. This does not create an observation or a sprite.
        let positions: BTreeMap<_, _> = self
            .data
            .habitats
            .occupants
            .values()
            .filter_map(|o| {
                let node = match o.location {
                    Occupancy::Underground(node) => Some(node),
                    Occupancy::Shelter(shelter) => self
                        .data
                        .habitats
                        .shelters
                        .get(&shelter)
                        .and_then(|s| s.burrow_node),
                    _ => None,
                }?;
                let n = self.data.habitats.nodes.get(&node)?;
                Some((o.organism, [n.position.x, n.position.y]))
            })
            .collect();
        let cell_mm = (CELL_METERS * 1000) as i32;
        for (cell, height) in unique {
            let min = [(cell % CW) as i32 * cell_mm, (cell / CW) as i32 * cell_mm];
            let patch = patch_at([min[0] as f32 / 1000., min[1] as f32 / 1000.]) as u32;
            let events = self
                .data
                .habitats
                .excavate_surface_at(patch, min, [min[0] + cell_mm, min[1] + cell_mm], height)
                .map_err(|e| e.to_string())?;
            for event in events {
                match event {
                    HabitatEvent::OccupantExposed { organism, .. } => {
                        if self
                            .data
                            .ecology
                            .cohorts
                            .iter()
                            .any(|c| c.id == organism && c.count > 0)
                        {
                            self.data
                                .ecology
                                .set_surface_observable(organism, true, 0)?;
                            if let (Some(actor), Some(&position)) = (
                                self.data.physical.actors.get_mut(&organism),
                                positions.get(&organism),
                            ) {
                                actor.position_mm = position;
                            }
                        }
                    }
                    HabitatEvent::ClutchExposed { clutch, .. } => {
                        // Collapse destroys this exact nest's remaining viable
                        // clutch. A blast may already have destroyed it; never
                        // debit another clutch or charge the same loss twice.
                        if self.data.ecology.clutches.iter().any(|c| c.id == clutch) {
                            self.data.ecology.destroy_clutch(clutch)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        let d = &mut self.data;
        d.physical
            .synchronize_breeding_refuges(&mut d.ecology, &d.habitats)?;
        d.nav.refresh(&self.terrain)?;
        Ok(())
    }

    /// Apply a uniform blast damage budget to actual natural structures whose
    /// anchor cell centers fall inside the world-space radius. For reefs the
    /// budget erodes at most that many millimeters of finite solid, matching the
    /// habitat substrate model; dam damage is integrity points. Neither operation
    /// deletes water or instantly resizes a reservoir.
    pub fn damage_surface_habitats(
        &mut self,
        center: [f32; 2],
        radius: f32,
        damage: u32,
    ) -> Result<(), String> {
        if center.iter().any(|v| !v.is_finite()) || !radius.is_finite() || radius < 0. {
            return Err("Invalid habitat blast geometry".into());
        }
        if damage == 0 || radius == 0. {
            return Ok(());
        }
        // Validate saved site references before any damage mutation. Structures
        // without a physical site are not silently projected to a patch center.
        for &cell in self
            .data
            .physical
            .dam_sites
            .values()
            .chain(self.data.physical.reef_sites.values())
        {
            if cell >= CW * CH {
                return Err("Saved habitat blast site outside resident world".into());
            }
        }
        let within = |cell: usize| {
            let x = ((cell % CW) * CELL_METERS + CELL_METERS / 2) as f64;
            let y = ((cell / CW) * CELL_METERS + CELL_METERS / 2) as f64;
            let dx = x - f64::from(center[0]);
            let dy = y - f64::from(center[1]);
            dx * dx + dy * dy <= f64::from(radius) * f64::from(radius)
        };
        let dams: Vec<_> = self
            .data
            .physical
            .dam_sites
            .iter()
            .filter_map(|(&id, &cell)| {
                (within(cell)
                    && self
                        .data
                        .habitats
                        .dams
                        .get(&id)
                        .is_some_and(|d| d.integrity > 0))
                .then_some(id)
            })
            .collect();
        let reefs: Vec<_> = self
            .data
            .physical
            .reef_sites
            .iter()
            .filter_map(|(&id, &cell)| {
                self.data
                    .habitats
                    .reefs
                    .get(&id)
                    .filter(|r| within(cell) && r.solid_height_mm > 0)
                    .map(|r| (id, damage.min(r.solid_height_mm)))
            })
            .collect();
        if dams.is_empty() && reefs.is_empty() {
            return Ok(());
        }
        // Preflight finite debris additions and all hydraulic references. The
        // individual habitat methods perform their own exact material debit.
        let wood = dams.iter().try_fold(0u64, |sum, id| {
            let d = &self.data.habitats.dams[id];
            sum.checked_add(if damage >= d.integrity {
                d.invested_wood_units
            } else {
                0
            })
            .ok_or("Habitat blast wood overflow")
        })?;
        let mineral = reefs.iter().try_fold(0u64, |sum, &(_, amount)| {
            sum.checked_add(u64::from(amount))
                .ok_or("Habitat blast mineral overflow")
        })?;
        self.data
            .habitats
            .debris
            .wood_units
            .checked_add(wood)
            .ok_or("Habitat debris wood overflow")?;
        self.data
            .habitats
            .debris
            .mineral_units
            .checked_add(mineral)
            .ok_or("Habitat debris mineral overflow")?;
        if self
            .data
            .physical
            .dam_links
            .values()
            .flatten()
            .any(|&i| i >= self.data.river.water.links().len())
        {
            return Err("Saved habitat hydraulic link outside graph".into());
        }
        for id in dams {
            self.data
                .habitats
                .damage_dam(id, damage)
                .map_err(|e| e.to_string())?;
        }
        for (id, removed_height) in reefs {
            self.data
                .habitats
                .damage_reef(id, removed_height)
                .map_err(|e| e.to_string())?;
        }
        let d = &mut self.data;
        d.physical
            .reconcile_hydraulics(&d.habitats, &self.terrain, &mut d.river)?;

        // Replace the overlay, rather than subtracting reef height a second
        // time from raw water depths. Changed clearances invalidate cached
        // routes through NavGrid's normal semantic revision mechanism.
        let mut overlays = Vec::new();
        for (&id, &cell) in &d.physical.reef_sites {
            if let Some(reef) = d.habitats.reefs.get(&id) {
                let position = [
                    ((cell % CW) * CELL_METERS + CELL_METERS / 2) as f32,
                    ((cell / CW) * CELL_METERS + CELL_METERS / 2) as f32,
                ];
                let index = d
                    .nav
                    .index(position)
                    .ok_or("Physical reef outside navigation grid")?;
                overlays.push((index, reef.solid_height_mm));
            }
        }
        d.nav.replace_reefs(&overlays)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        habitat::{Materials, Point3Mm, WorkBudget},
        river::RiverFixture,
    };

    fn funds() -> (Materials, WorkBudget) {
        // Explicit finite test fixture supplies, never a runtime resource source.
        (
            Materials {
                wood_units: 20,
                mineral_units: 200,
                ..Default::default()
            },
            WorkBudget {
                energy: 10_000,
                work_ticks: 100_000,
            },
        )
    }

    #[test]
    fn local_committed_cut_reveals_only_its_real_room_without_new_animals_or_score() {
        let mut game = Campaign::new(78421, 2, 2).unwrap();
        let source = game
            .data
            .ecology
            .cohorts
            .iter()
            .find(|c| c.count >= 2)
            .unwrap()
            .id;
        let near = game.data.ecology.materialize_one(source).unwrap();
        let far = game.data.ecology.materialize_one(source).unwrap();
        let count_before = game.data.ecology.census().wild_animals;
        let (mut materials, mut effort) = funds();
        let mut rooms = Vec::new();
        for (id, x) in [(near, 1000), (far, 31000)] {
            game.data
                .ecology
                .cohorts
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .patch = 0;
            let start = Point3Mm::new(x, 1000, 1000);
            let entrance = game.data.habitats.add_entrance(0, start, 200, 2).unwrap();
            let (room, _, _) = game
                .data
                .habitats
                .excavate_passage(
                    entrance,
                    vec![start, Point3Mm::new(x + 1000, 1000, -100)],
                    200,
                    2,
                    1000,
                    100_000_000,
                    &mut materials,
                    &mut effort,
                )
                .unwrap();
            game.data
                .habitats
                .enter_burrow(id, 1, 100, entrance)
                .unwrap();
            game.data
                .habitats
                .travel_burrow(id, room, &mut effort)
                .unwrap();
            game.data
                .ecology
                .set_surface_observable(id, false, -1100)
                .unwrap();
            rooms.push(room);
        }
        // A stale edit must fail before releasing any occupancy.
        let actual = game.terrain.elevation_mm(0).unwrap();
        assert!(
            game.reconcile_surface_habitat_edits(&[(0, actual + 1)])
                .is_err()
        );
        assert_eq!(game.data.habitats.occupants.len(), 2);
        game.terrain.set_elevation_mm(0, 200).unwrap();
        game.reconcile_surface_habitat_edits(&[(0, 200), (0, 200)])
            .unwrap();
        assert!(game.data.habitats.nodes[&rooms[0]].collapsed);
        assert!(!game.data.habitats.nodes[&rooms[1]].collapsed);
        assert!(!game.data.habitats.occupants.contains_key(&near));
        assert!(game.data.habitats.occupants.contains_key(&far));
        assert!(
            game.data
                .ecology
                .cohorts
                .iter()
                .find(|c| c.id == near)
                .unwrap()
                .surface_observable
        );
        assert!(
            !game
                .data
                .ecology
                .cohorts
                .iter()
                .find(|c| c.id == far)
                .unwrap()
                .surface_observable
        );
        assert_eq!(game.data.ecology.census().wild_animals, count_before);
        assert_eq!(game.data.ecological_damage, [0, 0]);
        // Repeating an already reconciled surface cut cannot duplicate losses.
        game.reconcile_surface_habitat_edits(&[(0, 200)]).unwrap();
        assert_eq!(game.data.habitats.occupants.len(), 1);
        game.data.habitats.validate().unwrap();
    }

    #[test]
    fn blast_breaches_only_near_dam_erodes_reef_and_preserves_water_and_material() {
        let mut game = Campaign::new(78421, 2, 2).unwrap();
        let cell = RiverFixture::global_index(300).unwrap();
        let remote = RiverFixture::global_index(6000).unwrap();
        let center = [((cell % CW) * 8 + 4) as f32, ((cell / CW) * 8 + 4) as f32];
        let patch = patch_at(center) as u32;
        let link = game
            .data
            .river
            .water
            .links()
            .iter()
            .position(|e| e.a == 300 || e.b == 300)
            .unwrap();
        let baseline = game.data.river.water.links()[link].barrier;
        let (mut materials, mut effort) = funds();
        let crest = game.terrain.elevation_mm(cell).unwrap() + 1000;
        let dam = game.data.habitats.begin_dam(patch, crest, 10).unwrap();
        game.data
            .habitats
            .build_dam(dam, 10, &mut materials, &mut effort)
            .unwrap();
        let distant = game
            .data
            .habitats
            .begin_dam(
                patch_at([((remote % CW) * 8) as f32, ((remote / CW) * 8) as f32]) as u32,
                crest,
                10,
            )
            .unwrap();
        game.data
            .habitats
            .build_dam(distant, 10, &mut materials, &mut effort)
            .unwrap();
        game.data.physical.dam_sites.insert(dam, cell);
        game.data.physical.dam_sites.insert(distant, remote);
        game.data.physical.dam_links.insert(dam, vec![link]);
        game.data
            .habitats
            .grow_reef(patch, 1000, 1, 200, &mut materials, &mut effort)
            .unwrap();
        let reef = *game.data.habitats.reefs.keys().next().unwrap();
        game.data.physical.reef_sites.insert(reef, cell);
        {
            let d = &mut game.data;
            d.physical
                .reconcile_hydraulics(&d.habitats, &game.terrain, &mut d.river)
                .unwrap();
        }
        assert_ne!(game.data.river.water.links()[link].barrier, baseline);
        let nav = game.data.nav.index(center).unwrap();
        game.data.nav.replace_reefs(&[(nav, 200)]).unwrap();
        let revision = game.data.nav.revision;
        let water = game.data.river.water.totals();
        let biology = game.data.ecology.census();
        game.damage_surface_habitats(center, 10., 100).unwrap();
        assert_eq!(game.data.habitats.dams[&dam].integrity, 0);
        assert_eq!(game.data.habitats.dams[&distant].integrity, 100);
        assert_eq!(game.data.habitats.debris.wood_units, 10);
        assert_eq!(game.data.habitats.reefs[&reef].solid_height_mm, 100);
        assert_eq!(game.data.habitats.debris.mineral_units, 100);
        assert_eq!(game.data.nav.cells[nav].reef_height_mm, 100);
        assert!(game.data.nav.revision > revision);
        assert_eq!(game.data.river.water.totals(), water);
        game.damage_surface_habitats(center, 10., 1000).unwrap();
        assert_eq!(game.data.river.water.links()[link].barrier, baseline);
        assert_eq!(game.data.habitats.debris.wood_units, 10);
        assert_eq!(game.data.habitats.debris.mineral_units, 200);
        assert_eq!(game.data.nav.cells[nav].reef_height_mm, 0);
        let revision = game.data.nav.revision;
        game.damage_surface_habitats(center, 10., 1000).unwrap();
        assert_eq!(game.data.nav.revision, revision);
        assert_eq!(game.data.river.water.totals(), water);
        assert_eq!(
            game.data.ecology.census().wild_animals,
            biology.wild_animals
        );
        assert_eq!(game.data.ecological_damage, [0, 0]);
        assert!(
            game.damage_surface_habitats([f32::NAN, 0.], 10., 5)
                .is_err()
        );
        game.data.habitats.validate().unwrap();
    }
}
