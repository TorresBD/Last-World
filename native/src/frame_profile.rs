//! Bounded diagnostics only. No simulation timing/ownership depends on these.
use serde::Serialize;

#[derive(Clone, Copy, Default, Debug, Serialize)]
pub struct FrameWork {
    pub frame: u64,
    pub tick: u64,
    pub event_gap_ms: f64,
    pub input_ms: f64,
    pub surface_acquire_ms: f64,
    pub simulation_ms: f64,
    pub upload_ms: f64,
    pub encode_submit_ms: f64,
    pub present_ms: f64,
    pub capture_ms: f64,
    pub reporting_ms: f64,
    pub total_work_ms: f64,
}
impl FrameWork {
    pub fn slow(&self) -> bool { self.total_work_ms + self.event_gap_ms >= 100. }
}

#[derive(Default)]
pub struct FrameProfile {
    pub count: u64,
    maxima: [f64; 10],
    sums: [f64; 10],
}
impl FrameProfile {
    pub fn record(&mut self, frame: &FrameWork) {
        let values = [frame.event_gap_ms,frame.input_ms,frame.surface_acquire_ms,frame.simulation_ms,frame.upload_ms,frame.encode_submit_ms,frame.present_ms,frame.capture_ms,frame.reporting_ms,frame.total_work_ms];
        self.count = self.count.checked_add(1).expect("Frame diagnostic counter overflow");
        for (i,v) in values.into_iter().enumerate() {
            self.maxima[i] = self.maxima[i].max(v);
            self.sums[i] += v;
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        let names = ["event_gap","input","surface_acquire","simulation","upload","encode_submit","present","capture","reporting","total_work"];
        let mut stages = serde_json::Map::new();
        for (i,name) in names.into_iter().enumerate() {
            stages.insert(name.to_owned(),serde_json::json!({"max_ms":self.maxima[i],"mean_ms":self.sums[i]/self.count.max(1) as f64}));
        }
        serde_json::json!({"frames":self.count,"stages":stages,"scope":"CPU wall durations; event_gap includes OS/event scheduling; no GPU timestamps"})
    }
    pub fn clear(&mut self) { *self = Self::default(); }
}
