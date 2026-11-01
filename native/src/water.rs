//! CPU-owned finite water storage and local surface/channel connections.
//!
//! This is a deliberately simple, dissipative storage/flow model, not a full
//! shallow-water or Navier–Stokes solver. Each call represents one 20 Hz game
//! tick. Callers own the authoritative clock and which regions are resident.
//! All requests read the same snapshot, then donor budgets are apportioned and
//! committed together. A newly built dam changes connections, never volumes.
use std::fmt;
use serde::{Serialize, Deserialize};

const HZ: u128 = crate::clock::TACTICAL_HZ as u128;
const PERMILLE: u128 = 1_000;
const RATE_DENOMINATOR: u128 = HZ * PERMILLE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageCell {
    pub bed_mm: i32,
    pub area_m2: u32,
    pub volume_liters: u64,
}

#[cfg(test)]
mod exchange_tests {
    use super::*;
    #[test]
    fn cross_owner_exchange_is_exact_and_failing_batch_is_atomic() {
        let mut graph = WaterGraph::new(
            vec![
                StorageCell {
                    bed_mm: 0,
                    area_m2: 1,
                    volume_liters: 10,
                },
                StorageCell {
                    bed_mm: 0,
                    area_m2: 1,
                    volume_liters: 20,
                },
            ],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let before = graph.totals();
        let cells = graph.cells().to_vec();
        assert_eq!(
            graph.exchange_cells(&[5, 21], &[0, 0]),
            Err(WaterError::InsufficientWater(1))
        );
        assert_eq!(graph.totals(), before);
        assert_eq!(graph.cells(), cells);
        graph.exchange_cells(&[5, 0], &[0, 2]).unwrap();
        assert_eq!(graph.totals().extracted_liters, 5);
        assert_eq!(graph.totals().returned_liters, 2);
        assert!(graph.balanced());
        graph.withdraw(1, 2).unwrap();
        graph.return_water(0, 2).unwrap();
        assert!(graph.balanced());
    }
    #[test]
    fn exposed_transport_snapshot_is_before_transfer_and_does_not_change_on_exchange() {
        let mut graph = WaterGraph::new(
            vec![StorageCell {
                bed_mm: 0,
                area_m2: 1,
                volume_liters: 10,
            }],
            vec![],
            vec![],
            vec![Outlet {
                cell: 0,
                level_mm: 0,
                conductance_lps_per_mm: 100,
            }],
        )
        .unwrap();
        graph.step().unwrap();
        assert_eq!(graph.last_transport_volumes(), &[10]);
        assert_eq!(graph.last_outlet_flow_liters(0), Some(5));
        assert_eq!(graph.transport_generation(), 1);
        graph.withdraw(0, 1).unwrap();
        assert_eq!(graph.last_transport_volumes(), &[10]);
        assert_eq!(graph.transport_generation(), 1);
        assert!(graph.balanced());
    }
}

impl StorageCell {
    /// One liter spread over one square meter is one millimeter of depth.
    /// Sub-millimeter water remains owned in volume_liters, not discarded.
    pub fn depth_mm(&self) -> u64 {
        self.volume_liters / self.area_m2 as u64
    }

    pub fn head_mm(&self) -> i128 {
        self.bed_mm as i128 + self.depth_mm() as i128
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Barrier {
    pub crest_mm: i32,
    /// 0 is sealed below its crest; 1000 is fully permeable.
    pub permeability_permille: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowLink {
    pub a: usize,
    pub b: usize,
    pub conductance_lps_per_mm: u32,
    pub barrier: Option<Barrier>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inlet {
    pub cell: usize,
    /// Explicit external input, e.g. upstream catchment supply.
    pub liters_per_second: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outlet {
    pub cell: usize,
    pub level_mm: i32,
    /// Outlets remove finite available water; they never inject reverse flow.
    pub conductance_lps_per_mm: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaterStep {
    pub added_liters: u128,
    pub drained_liters: u128,
    pub transferred_liters: u128,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaterTotals {
    pub initial_liters: u128,
    pub stored_liters: u128,
    pub added_liters: u128,
    pub drained_liters: u128,
    /// Transfers to another authoritative owner, e.g. finite soil moisture.
    pub extracted_liters: u128,
    pub returned_liters: u128,
}

impl WaterTotals {
    pub fn balanced(&self) -> bool {
        match (
            self.initial_liters
                .checked_add(self.added_liters)
                .and_then(|v| v.checked_add(self.returned_liters)),
            self.stored_liters
                .checked_add(self.drained_liters)
                .and_then(|v| v.checked_add(self.extracted_liters)),
        ) {
            (Some(inputs), Some(outputs)) => inputs == outputs,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaterError {
    EmptyStorage,
    InvalidArea(usize),
    InvalidCell(usize),
    InvalidLink(usize),
    InvalidInlet(usize),
    InvalidOutlet(usize),
    InvalidPermeability(u16),
    Overflow(&'static str),
    ConservationFailure,
    InsufficientWater(usize),
    InvalidExchange,
}

impl fmt::Display for WaterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStorage => write!(f, "Water graph requires at least one storage cell"),
            Self::InvalidArea(i) => write!(f, "Water cell {i} has zero surface area"),
            Self::InvalidCell(i) => write!(f, "Invalid water cell index {i}"),
            Self::InvalidLink(i) => write!(f, "Invalid water link index/endpoints {i}"),
            Self::InvalidInlet(i) => write!(f, "Invalid water inlet {i}"),
            Self::InvalidOutlet(i) => write!(f, "Invalid water outlet {i}"),
            Self::InvalidPermeability(p) => {
                write!(f, "Invalid dam permeability {p}; expected 0..=1000")
            }
            Self::Overflow(context) => write!(f, "Water arithmetic overflow: {context}"),
            Self::ConservationFailure => write!(f, "Water transfer did not conserve owned volume"),
            Self::InsufficientWater(i) => {
                write!(f, "Water cell {i} cannot fund an exact withdrawal")
            }
            Self::InvalidExchange => write!(f, "Water exchange arrays must match storage cells"),
        }
    }
}

impl std::error::Error for WaterError {}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
struct Request {
    from: usize,
    to: Option<usize>,
    requested: u64,
    above_crest: u64,
    actual: u64,
    direction: i8,
}

/// Scratch arrays are allocated once with the graph and reused every tick.
/// They also stage all potentially fallible work before authoritative commit.
#[derive(Clone, Serialize, Deserialize)]
struct Scratch {
    volumes: Vec<u64>,
    transport_volumes: Vec<u64>,
    incoming: Vec<u128>,
    outgoing_requests: Vec<u128>,
    outgoing_count: Vec<usize>,
    assigned: Vec<u64>,
    remaining: Vec<u64>,
    requests: Vec<Request>,
    inlet_remainders: Vec<u32>,
    link_remainders: Vec<u32>,
    link_directions: Vec<i8>,
    outlet_remainders: Vec<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WaterGraph {
    cells: Vec<StorageCell>,
    links: Vec<FlowLink>,
    inlets: Vec<Inlet>,
    outlets: Vec<Outlet>,
    inlet_remainders: Vec<u32>,
    link_remainders: Vec<u32>,
    link_directions: Vec<i8>,
    outlet_remainders: Vec<u32>,
    last_link_flows: Vec<i128>,
    last_outlet_flows: Vec<u64>,
    last_transport_volumes: Vec<u64>,
    transport_generation: u64,
    totals: WaterTotals,
    scratch: Scratch,
}

fn add(a: u128, b: u128, context: &'static str) -> Result<u128, WaterError> {
    a.checked_add(b).ok_or(WaterError::Overflow(context))
}

fn mul(a: u128, b: u128, context: &'static str) -> Result<u128, WaterError> {
    a.checked_mul(b).ok_or(WaterError::Overflow(context))
}

/// Water below a connection crest is valid retained stock, not a negative
/// inventory to clamp. Only the excess is eligible to leave through that link.
fn volume_above_crest(stored: u64, retained: u128) -> u128 {
    if stored as u128 > retained {
        stored as u128 - retained
    } else {
        0
    }
}

impl WaterGraph {
    pub fn new(
        cells: Vec<StorageCell>,
        links: Vec<FlowLink>,
        inlets: Vec<Inlet>,
        outlets: Vec<Outlet>,
    ) -> Result<Self, WaterError> {
        if cells.is_empty() {
            return Err(WaterError::EmptyStorage);
        }
        let mut initial = 0u128;
        for (i, cell) in cells.iter().enumerate() {
            if cell.area_m2 == 0 {
                return Err(WaterError::InvalidArea(i));
            }
            initial = add(initial, cell.volume_liters as u128, "initial volume")?;
        }
        for (i, link) in links.iter().enumerate() {
            if link.a >= cells.len() || link.b >= cells.len() || link.a == link.b {
                return Err(WaterError::InvalidLink(i));
            }
            if let Some(b) = link.barrier {
                Self::validate_barrier(b)?;
            }
        }
        for (i, inlet) in inlets.iter().enumerate() {
            if inlet.cell >= cells.len() {
                return Err(WaterError::InvalidInlet(i));
            }
        }
        for (i, outlet) in outlets.iter().enumerate() {
            if outlet.cell >= cells.len() {
                return Err(WaterError::InvalidOutlet(i));
            }
        }
        let n = cells.len();
        let e = links.len();
        let a = inlets.len();
        let b = outlets.len();
        Ok(Self {
            cells,
            links,
            inlets,
            outlets,
            inlet_remainders: vec![0; a],
            link_remainders: vec![0; e],
            link_directions: vec![0; e],
            outlet_remainders: vec![0; b],
            last_link_flows: vec![0; e],
            last_outlet_flows: vec![0; b],
            last_transport_volumes: vec![0; n],
            transport_generation: 0,
            totals: WaterTotals {
                initial_liters: initial,
                stored_liters: initial,
                ..WaterTotals::default()
            },
            scratch: Scratch {
                volumes: vec![0; n],
                transport_volumes: vec![0; n],
                incoming: vec![0; n],
                outgoing_requests: vec![0; n],
                outgoing_count: vec![0; n],
                assigned: vec![0; n],
                remaining: vec![0; n],
                requests: vec![Request::default(); e + b],
                inlet_remainders: vec![0; a],
                link_remainders: vec![0; e],
                link_directions: vec![0; e],
                outlet_remainders: vec![0; b],
            },
        })
    }

    fn validate_barrier(barrier: Barrier) -> Result<(), WaterError> {
        if barrier.permeability_permille > 1_000 {
            return Err(WaterError::InvalidPermeability(
                barrier.permeability_permille,
            ));
        }
        Ok(())
    }

    pub fn cells(&self) -> &[StorageCell] {
        &self.cells
    }
    pub fn links(&self) -> &[FlowLink] {
        &self.links
    }
    pub fn totals(&self) -> WaterTotals {
        self.totals
    }
    pub fn balanced(&self) -> bool {
        self.totals.balanced()
    }
    /// Named precipitation boundary, distinct from a transfer from soil/biology.
    pub fn add_boundary_water(&mut self, index: usize, liters: u64) -> Result<(), WaterError> {
        let cell = self.cells.get(index).ok_or(WaterError::InvalidCell(index))?;
        let volume = cell.volume_liters.checked_add(liters).ok_or(WaterError::Overflow("rain storage"))?;
        let added = add(self.totals.added_liters, liters as u128, "rain input")?;
        let stored = add(self.totals.stored_liters, liters as u128, "rain stored")?;
        self.cells[index].volume_liters = volume;
        self.totals.added_liters = added;
        self.totals.stored_liters = stored;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), WaterError> {
        Self::new(self.cells.clone(),self.links.clone(),self.inlets.clone(),self.outlets.clone())?;
        let n=self.cells.len(); let e=self.links.len(); let a=self.inlets.len(); let b=self.outlets.len();
        if self.inlet_remainders.len()!=a || self.link_remainders.len()!=e || self.link_directions.len()!=e || self.outlet_remainders.len()!=b || self.last_link_flows.len()!=e || self.last_outlet_flows.len()!=b || self.last_transport_volumes.len()!=n
            || self.scratch.volumes.len()!=n || self.scratch.transport_volumes.len()!=n || self.scratch.incoming.len()!=n || self.scratch.outgoing_requests.len()!=n || self.scratch.outgoing_count.len()!=n || self.scratch.assigned.len()!=n || self.scratch.remaining.len()!=n || self.scratch.requests.len()!=e+b || self.scratch.inlet_remainders.len()!=a || self.scratch.link_remainders.len()!=e || self.scratch.link_directions.len()!=e || self.scratch.outlet_remainders.len()!=b {
            return Err(WaterError::InvalidExchange);
        }
        let total: u128=self.cells.iter().map(|c|c.volume_liters as u128).sum();
        if total!=self.totals.stored_liters || !self.balanced() { return Err(WaterError::ConservationFailure); }
        Ok(())
    }
    pub fn outlets(&self) -> &[Outlet] {
        &self.outlets
    }
    /// Snapshot after external inlets and before this generation's transfers.
    pub fn last_transport_volumes(&self) -> &[u64] {
        &self.last_transport_volumes
    }
    pub fn last_outlet_flow_liters(&self, index: usize) -> Option<u64> {
        self.last_outlet_flows.get(index).copied()
    }
    /// A transfer revision, not a second game clock. Guards against applying
    /// the same water-flow snapshot twice to sediment or biomass.
    pub fn transport_generation(&self) -> u64 {
        self.transport_generation
    }

    pub fn withdraw(&mut self, index: usize, liters: u64) -> Result<(), WaterError> {
        let current = self
            .cells
            .get(index)
            .ok_or(WaterError::InvalidCell(index))?
            .volume_liters;
        let next = current
            .checked_sub(liters)
            .ok_or(WaterError::InsufficientWater(index))?;
        let extracted = add(
            self.totals.extracted_liters,
            liters as u128,
            "external withdrawal",
        )?;
        self.cells[index].volume_liters = next;
        self.totals.stored_liters -= liters as u128;
        self.totals.extracted_liters = extracted;
        Ok(())
    }
    pub fn return_water(&mut self, index: usize, liters: u64) -> Result<(), WaterError> {
        let current = self
            .cells
            .get(index)
            .ok_or(WaterError::InvalidCell(index))?
            .volume_liters;
        let next = current
            .checked_add(liters)
            .ok_or(WaterError::Overflow("returned storage"))?;
        let returned = add(
            self.totals.returned_liters,
            liters as u128,
            "external return",
        )?;
        let stored = add(self.totals.stored_liters, liters as u128, "returned total")?;
        self.cells[index].volume_liters = next;
        self.totals.stored_liters = stored;
        self.totals.returned_liters = returned;
        Ok(())
    }
    /// Validate and stage the complete cross-owner exchange before committing.
    /// The caller commits its matching receiving/sending stocks after success.
    pub fn exchange_cells(
        &mut self,
        withdrawals: &[u64],
        returns: &[u64],
    ) -> Result<(), WaterError> {
        if withdrawals.len() != self.cells.len() || returns.len() != self.cells.len() {
            return Err(WaterError::InvalidExchange);
        }
        let mut extracted = self.totals.extracted_liters;
        let mut returned = self.totals.returned_liters;
        let mut stored = 0u128;
        for i in 0..self.cells.len() {
            let after = self.cells[i]
                .volume_liters
                .checked_sub(withdrawals[i])
                .ok_or(WaterError::InsufficientWater(i))?
                .checked_add(returns[i])
                .ok_or(WaterError::Overflow("exchange storage"))?;
            self.scratch.volumes[i] = after;
            extracted = add(extracted, withdrawals[i] as u128, "exchange extraction")?;
            returned = add(returned, returns[i] as u128, "exchange return")?;
            stored = add(stored, after as u128, "exchange total")?;
        }
        let totals = WaterTotals {
            stored_liters: stored,
            extracted_liters: extracted,
            returned_liters: returned,
            ..self.totals
        };
        if !totals.balanced() {
            return Err(WaterError::ConservationFailure);
        }
        for (cell, &volume) in self.cells.iter_mut().zip(&self.scratch.volumes) {
            cell.volume_liters = volume;
        }
        self.totals = totals;
        Ok(())
    }

    /// Positive means a -> b. This is liters transferred during the last tick,
    /// not a second inventory or an instantaneous velocity measurement.
    pub fn last_link_flow_liters(&self, index: usize) -> Option<i128> {
        self.last_link_flows.get(index).copied()
    }

    pub fn set_barrier(
        &mut self,
        index: usize,
        barrier: Option<Barrier>,
    ) -> Result<(), WaterError> {
        if let Some(b) = barrier {
            Self::validate_barrier(b)?;
        }
        let link = self
            .links
            .get_mut(index)
            .ok_or(WaterError::InvalidLink(index))?;
        link.barrier = barrier;
        self.link_remainders[index] = 0;
        Ok(())
    }

    /// Geometry changes preserve owned volume. Redistribution happens on later
    /// ordinary ticks, not while editing the height or building a dam.
    pub fn set_bed(&mut self, index: usize, bed_mm: i32) -> Result<(), WaterError> {
        self.cells
            .get_mut(index)
            .ok_or(WaterError::InvalidCell(index))?
            .bed_mm = bed_mm;
        Ok(())
    }

    pub fn step(&mut self) -> Result<WaterStep, WaterError> {
        let s = &mut self.scratch;
        s.incoming.fill(0);
        s.outgoing_requests.fill(0);
        s.outgoing_count.fill(0);
        s.assigned.fill(0);
        for (to, cell) in s.volumes.iter_mut().zip(&self.cells) {
            *to = cell.volume_liters;
        }
        s.inlet_remainders.copy_from_slice(&self.inlet_remainders);
        s.link_remainders.copy_from_slice(&self.link_remainders);
        s.link_directions.copy_from_slice(&self.link_directions);
        s.outlet_remainders.copy_from_slice(&self.outlet_remainders);
        let mut result = WaterStep::default();

        // Sources are named external inputs. Only whole liters enter ownership;
        // fractional rates carry forward rather than vanishing each tick.
        for (i, inlet) in self.inlets.iter().enumerate() {
            let amount = inlet.liters_per_second as u128 + s.inlet_remainders[i] as u128;
            let liters = (amount / HZ) as u64;
            s.inlet_remainders[i] = (amount % HZ) as u32;
            s.volumes[inlet.cell] = s.volumes[inlet.cell]
                .checked_add(liters)
                .ok_or(WaterError::Overflow("inlet storage"))?;
            result.added_liters = add(result.added_liters, liters as u128, "tick input")?;
        }
        s.transport_volumes.copy_from_slice(&s.volumes);

        for (i, link) in self.links.iter().enumerate() {
            // Dry connections cannot transport anything. Avoid the expensive
            // head/discharge/equalization arithmetic across inactive ground.
            if s.volumes[link.a] == 0 && s.volumes[link.b] == 0 {
                s.link_remainders[i] = 0;
                s.link_directions[i] = 0;
                s.requests[i] = Request {
                    from: link.a,
                    to: Some(link.b),
                    ..Request::default()
                };
                continue;
            }
            let a = self.cells[link.a];
            let b = self.cells[link.b];
            let ah = a.bed_mm as i128 + (s.volumes[link.a] / a.area_m2 as u64) as i128;
            let bh = b.bed_mm as i128 + (s.volumes[link.b] / b.area_m2 as u64) as i128;
            let (from, to, high, low, direction) = if ah >= bh {
                (link.a, link.b, ah, bh, 1)
            } else {
                (link.b, link.a, bh, ah, -1)
            };
            if direction != s.link_directions[i] {
                s.link_remainders[i] = 0;
            }
            s.link_directions[i] = direction;
            let base_crest = a.bed_mm.max(b.bed_mm) as i128;
            let free_head = (high - low.max(base_crest)).max(0) as u128;
            let (spill_head, permeability, active_crest) = match link.barrier {
                Some(barrier) => {
                    let crest = (barrier.crest_mm as i128).max(base_crest);
                    (
                        (high - low.max(crest)).max(0) as u128,
                        barrier.permeability_permille as u128,
                        if barrier.permeability_permille == 0 {
                            crest
                        } else {
                            base_crest
                        },
                    )
                }
                None => (free_head, PERMILLE, base_crest),
            };
            let below_crest_head = free_head - spill_head;
            let effective_head = add(
                mul(spill_head, PERMILLE, "spill head")?,
                mul(below_crest_head, permeability, "seep head")?,
                "effective head",
            )?;
            let numerator = add(
                mul(
                    effective_head,
                    link.conductance_lps_per_mm as u128,
                    "link rate",
                )?,
                s.link_remainders[i] as u128,
                "link remainder",
            )?;
            s.link_remainders[i] = if effective_head == 0 {
                0
            } else {
                (numerator % RATE_DENOMINATOR) as u32
            };
            let donor = self.cells[from];
            let receiver = self.cells[to];
            let driving_head = (high - low.max(active_crest)).max(0) as u128;
            let equalize = mul(
                mul(driving_head, donor.area_m2 as u128, "equalization area")?,
                receiver.area_m2 as u128,
                "equalization volume",
            )? / (donor.area_m2 as u128 + receiver.area_m2 as u128);
            let retained = mul(
                (active_crest - donor.bed_mm as i128).max(0) as u128,
                donor.area_m2 as u128,
                "crest retention",
            )?;
            let above_crest = volume_above_crest(s.volumes[from], retained);
            let requested = (numerator / RATE_DENOMINATOR)
                .min(equalize)
                .min(above_crest) as u64;
            s.requests[i] = Request {
                from,
                to: Some(to),
                requested,
                above_crest: above_crest as u64,
                actual: 0,
                direction,
            };
        }
        for (i, outlet) in self.outlets.iter().enumerate() {
            let cell = self.cells[outlet.cell];
            let head = cell.bed_mm as i128 + (s.volumes[outlet.cell] / cell.area_m2 as u64) as i128;
            let level = (outlet.level_mm as i128).max(cell.bed_mm as i128);
            let excess = (head - level).max(0) as u128;
            let numerator = add(
                mul(excess, outlet.conductance_lps_per_mm as u128, "outlet rate")?,
                s.outlet_remainders[i] as u128,
                "outlet remainder",
            )?;
            s.outlet_remainders[i] = if excess == 0 {
                0
            } else {
                (numerator % HZ) as u32
            };
            let retained = mul(
                (level - cell.bed_mm as i128) as u128,
                cell.area_m2 as u128,
                "outlet retention",
            )?;
            let requested =
                (numerator / HZ).min(volume_above_crest(s.volumes[outlet.cell], retained)) as u64;
            s.requests[self.links.len() + i] = Request {
                from: outlet.cell,
                to: None,
                requested,
                above_crest: volume_above_crest(s.volumes[outlet.cell], retained) as u64,
                actual: 0,
                direction: 1,
            };
        }

        // Different outlet crests share overlapping donor headbands. A total
        // volume budget alone could let several high outlets jointly drain
        // below their crest. Give each active connection at most one equal
        // share of its own eligible headband, then allocate the shared budget.
        // This conservative limiter permits lower outlets to drain lower stock
        // without pledging the same above-dam volume to several high outlets.
        for request in &s.requests {
            if request.requested != 0 {
                s.outgoing_count[request.from] += 1;
            }
        }
        for request in &mut s.requests {
            if request.requested != 0 {
                request.requested = request
                    .requested
                    .min(request.above_crest / s.outgoing_count[request.from] as u64);
            }
            s.outgoing_requests[request.from] = add(
                s.outgoing_requests[request.from],
                request.requested as u128,
                "donor requests",
            )?;
        }

        // A dissipative half-volume donor budget bounds aggregate local motion
        // even when several steep links simultaneously ask for the same stock.
        // This is a numerical stability policy, not deleting or freezing water.
        for (i, volume) in s.volumes.iter().copied().enumerate() {
            s.remaining[i] = volume / 2 + volume % 2;
        }
        for request in &mut s.requests {
            let demand = s.outgoing_requests[request.from];
            let budget = s.remaining[request.from];
            request.actual = if demand <= budget as u128 {
                request.requested
            } else {
                (mul(
                    request.requested as u128,
                    budget as u128,
                    "donor allocation",
                )? / demand) as u64
            };
            s.assigned[request.from] = s.assigned[request.from]
                .checked_add(request.actual)
                .ok_or(WaterError::Overflow("assigned donor flow"))?;
        }
        for i in 0..s.remaining.len() {
            s.remaining[i] -= s.assigned[i];
        }
        // Proportional floors leave fewer than one liter per outgoing request.
        // Stable link IDs resolve those rounding ties deterministically.
        for request in &mut s.requests {
            if request.actual < request.requested && s.remaining[request.from] != 0 {
                request.actual += 1;
                s.assigned[request.from] += 1;
                s.remaining[request.from] -= 1;
            }
            if let Some(to) = request.to {
                s.incoming[to] = add(s.incoming[to], request.actual as u128, "incoming transfers")?;
                result.transferred_liters = add(
                    result.transferred_liters,
                    request.actual as u128,
                    "tick transfer total",
                )?;
            } else {
                result.drained_liters = add(
                    result.drained_liters,
                    request.actual as u128,
                    "tick drainage",
                )?;
            }
        }

        let mut stored = 0u128;
        for i in 0..self.cells.len() {
            let final_volume = add(s.volumes[i] as u128, s.incoming[i], "final storage")?
                .checked_sub(s.assigned[i] as u128)
                .ok_or(WaterError::ConservationFailure)?;
            s.volumes[i] =
                u64::try_from(final_volume).map_err(|_| WaterError::Overflow("cell storage"))?;
            stored = add(stored, final_volume, "stored total")?;
        }
        let totals = WaterTotals {
            initial_liters: self.totals.initial_liters,
            stored_liters: stored,
            added_liters: add(
                self.totals.added_liters,
                result.added_liters,
                "cumulative inputs",
            )?,
            drained_liters: add(
                self.totals.drained_liters,
                result.drained_liters,
                "cumulative drainage",
            )?,
            extracted_liters: self.totals.extracted_liters,
            returned_liters: self.totals.returned_liters,
        };
        if !totals.balanced() {
            return Err(WaterError::ConservationFailure);
        }
        let generation = self
            .transport_generation
            .checked_add(1)
            .ok_or(WaterError::Overflow("transport generation"))?;
        // Every fallible calculation above has succeeded. Commit exactly once.
        for (cell, volume) in self.cells.iter_mut().zip(&s.volumes) {
            cell.volume_liters = *volume;
        }
        self.inlet_remainders.copy_from_slice(&s.inlet_remainders);
        self.link_remainders.copy_from_slice(&s.link_remainders);
        self.link_directions.copy_from_slice(&s.link_directions);
        self.outlet_remainders.copy_from_slice(&s.outlet_remainders);
        for (flow, request) in self.last_link_flows.iter_mut().zip(&s.requests) {
            *flow = request.actual as i128 * request.direction as i128;
        }
        for (flow, request) in self
            .last_outlet_flows
            .iter_mut()
            .zip(&s.requests[self.links.len()..])
        {
            *flow = request.actual;
        }
        self.last_transport_volumes
            .copy_from_slice(&s.transport_volumes);
        self.transport_generation = generation;
        self.totals = totals;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(bed_mm: i32, volume_liters: u64) -> StorageCell {
        StorageCell {
            bed_mm,
            area_m2: 1,
            volume_liters,
        }
    }
    fn link(a: usize, b: usize, conductance: u32) -> FlowLink {
        FlowLink {
            a,
            b,
            conductance_lps_per_mm: conductance,
            barrier: None,
        }
    }

    #[test]
    fn finite_rates_and_transfers_preserve_exact_water_balance() {
        let mut graph = WaterGraph::new(
            vec![cell(100, 100), cell(0, 0)],
            vec![link(0, 1, 2)],
            vec![Inlet {
                cell: 0,
                liters_per_second: 47,
            }],
            vec![Outlet {
                cell: 1,
                level_mm: 0,
                conductance_lps_per_mm: 3,
            }],
        )
        .unwrap();
        for _ in 0..1_000 {
            graph.step().unwrap();
            assert!(graph.balanced());
        }
        let totals = graph.totals();
        assert_eq!(totals.added_liters, 2_350);
        assert_eq!(
            totals.initial_liters + totals.added_liters,
            totals.stored_liters + totals.drained_liters
        );
        assert!(totals.drained_liters > 0);
    }

    #[test]
    fn dam_fills_gradually_throttles_downstream_and_inundates_connected_bank() {
        fn river() -> WaterGraph {
            WaterGraph::new(
                vec![cell(0, 100), cell(0, 100), cell(150, 0)],
                vec![link(0, 1, 5), link(0, 2, 5)],
                vec![Inlet {
                    cell: 0,
                    liters_per_second: 40,
                }],
                vec![Outlet {
                    cell: 1,
                    level_mm: 0,
                    conductance_lps_per_mm: 5,
                }],
            )
            .unwrap()
        }
        let mut dammed = river();
        let mut open = river();
        let before = dammed.cells().to_vec();
        dammed
            .set_barrier(
                0,
                Some(Barrier {
                    crest_mm: 200,
                    permeability_permille: 0,
                }),
            )
            .unwrap();
        assert_eq!(dammed.cells(), before); // construction creates no water
        dammed.step().unwrap();
        assert_eq!(dammed.cells()[0].volume_liters, 102);
        assert_eq!(dammed.cells()[2].volume_liters, 0); // no instant reservoir
        for _ in 0..199 {
            dammed.step().unwrap();
        }
        for _ in 0..200 {
            open.step().unwrap();
        }
        assert!(dammed.cells()[0].head_mm() > open.cells()[0].head_mm());
        assert!(dammed.cells()[2].volume_liters > 0);
        assert_eq!(open.cells()[2].volume_liters, 0);
        assert!(dammed.totals().drained_liters < open.totals().drained_liters);
        assert!(dammed.balanced());
    }

    #[test]
    fn disconnected_low_ground_does_not_fill_through_a_high_ridge() {
        let mut graph = WaterGraph::new(
            vec![cell(0, 200), cell(500, 0), cell(0, 0)],
            vec![link(0, 1, 1_000), link(1, 2, 1_000)],
            vec![],
            vec![],
        )
        .unwrap();
        for _ in 0..100 {
            graph.step().unwrap();
        }
        assert_eq!(graph.cells()[2].volume_liters, 0);
        assert_eq!(graph.cells()[0].volume_liters, 200);
    }

    #[test]
    fn equal_water_heads_do_not_create_circulation() {
        let mut graph = WaterGraph::new(
            vec![cell(0, 100), cell(50, 50)],
            vec![link(0, 1, 1_000)],
            vec![],
            vec![],
        )
        .unwrap();
        let before = graph.cells().to_vec();
        for _ in 0..100 {
            assert_eq!(graph.step().unwrap().transferred_liters, 0);
        }
        assert_eq!(graph.cells(), before);
    }

    #[test]
    fn breach_opens_flow_but_does_not_teleport_or_delete_reservoir() {
        let mut graph = WaterGraph::new(
            vec![cell(0, 1_000), cell(0, 0)],
            vec![link(0, 1, 1_000)],
            vec![],
            vec![],
        )
        .unwrap();
        graph
            .set_barrier(
                0,
                Some(Barrier {
                    crest_mm: 1_000,
                    permeability_permille: 0,
                }),
            )
            .unwrap();
        graph.step().unwrap();
        graph.set_barrier(0, None).unwrap();
        assert_eq!(graph.cells()[0].volume_liters, 1_000);
        graph.step().unwrap();
        assert_eq!(graph.cells()[0].volume_liters, 500);
        assert_eq!(graph.cells()[1].volume_liters, 500);
        assert!(graph.balanced());
    }

    #[test]
    fn simultaneous_donors_cannot_spend_the_same_volume_twice() {
        let mut graph = WaterGraph::new(
            vec![
                cell(0, 1_000),
                cell(0, 0),
                cell(0, 0),
                cell(0, 0),
                cell(0, 0),
            ],
            vec![
                link(0, 1, 1_000),
                link(0, 2, 1_000),
                link(0, 3, 1_000),
                link(0, 4, 1_000),
            ],
            vec![],
            vec![],
        )
        .unwrap();
        graph.step().unwrap();
        assert_eq!(graph.cells()[0].volume_liters, 500);
        for cell in &graph.cells()[1..] {
            assert_eq!(cell.volume_liters, 125);
        }
        assert!(graph.balanced());
    }

    #[test]
    fn snapshots_prevent_double_distance_from_link_iteration_order() {
        let cells = vec![cell(0, 100), cell(0, 0), cell(0, 0)];
        let mut forward = WaterGraph::new(
            cells.clone(),
            vec![link(0, 1, 1_000), link(1, 2, 1_000)],
            vec![],
            vec![],
        )
        .unwrap();
        let mut reverse = WaterGraph::new(
            cells,
            vec![link(1, 2, 1_000), link(0, 1, 1_000)],
            vec![],
            vec![],
        )
        .unwrap();
        forward.step().unwrap();
        reverse.step().unwrap();
        assert_eq!(forward.cells()[2].volume_liters, 0);
        assert_eq!(forward.cells(), reverse.cells());
        for _ in 0..100 {
            forward.step().unwrap();
            reverse.step().unwrap();
        }
        assert_eq!(forward.cells(), reverse.cells());
    }

    #[test]
    fn multiple_sealed_outlets_cannot_jointly_drain_below_their_crest() {
        let area_cell = |volume_liters| StorageCell {
            bed_mm: 0,
            area_m2: 100,
            volume_liters,
        };
        let mut graph = WaterGraph::new(
            vec![area_cell(100_000), area_cell(0), area_cell(0), area_cell(0)],
            vec![link(0, 1, 10_000), link(0, 2, 10_000), link(0, 3, 10_000)],
            vec![],
            vec![],
        )
        .unwrap();
        for index in 0..3 {
            graph
                .set_barrier(
                    index,
                    Some(Barrier {
                        crest_mm: 900,
                        permeability_permille: 0,
                    }),
                )
                .unwrap();
        }
        for _ in 0..100 {
            graph.step().unwrap();
            assert!(graph.cells()[0].volume_liters >= 90_000);
            assert!(graph.balanced());
        }
        assert!(graph.cells()[1..].iter().any(|cell| cell.volume_liters > 0));
    }

    #[test]
    fn high_crest_does_not_prevent_a_lower_open_connection_from_draining() {
        let area_cell = |volume_liters| StorageCell {
            bed_mm: 0,
            area_m2: 100,
            volume_liters,
        };
        let mut graph = WaterGraph::new(
            vec![area_cell(100_000), area_cell(0), area_cell(0)],
            vec![link(0, 1, 10_000), link(0, 2, 10_000)],
            vec![],
            vec![],
        )
        .unwrap();
        graph
            .set_barrier(
                0,
                Some(Barrier {
                    crest_mm: 900,
                    permeability_permille: 0,
                }),
            )
            .unwrap();
        graph.step().unwrap();
        assert!(graph.cells()[0].volume_liters < 90_000);
        assert!(graph.cells()[1].volume_liters <= 10_000);
        assert!(graph.cells()[2].volume_liters > graph.cells()[1].volume_liters);
        assert!(graph.balanced());
    }

    #[test]
    fn invalid_settings_and_overflow_are_explicit_and_do_not_commit() {
        assert!(matches!(
            WaterGraph::new(
                vec![StorageCell {
                    bed_mm: 0,
                    area_m2: 0,
                    volume_liters: 0
                }],
                vec![],
                vec![],
                vec![]
            ),
            Err(WaterError::InvalidArea(0))
        ));
        let mut graph = WaterGraph::new(
            vec![cell(0, u64::MAX)],
            vec![],
            vec![Inlet {
                cell: 0,
                liters_per_second: 20,
            }],
            vec![],
        )
        .unwrap();
        let before = graph.totals();
        assert!(matches!(
            graph.step(),
            Err(WaterError::Overflow("inlet storage"))
        ));
        assert_eq!(graph.totals(), before);
        assert_eq!(graph.cells()[0].volume_liters, u64::MAX);
        assert_eq!(graph.set_barrier(0, None), Err(WaterError::InvalidLink(0)));
    }
}
