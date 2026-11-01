//! Last-observed presentation for the resident fixture. No gameplay authority.
//! 32m observation pages update independently; radar never refreshes a page.
use crate::{terrain::{Terrain, Cell, CW, CH, WW}, gpu::Object, campaign_support::VISIBLE};
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct VisualMemory {
    pub cells: Vec<Cell>,
    pub rgba: Vec<u8>,
    pub objects: Vec<Vec<Object>>,
    #[serde(skip)]
    pub previous_masks: Vec<u32>,
    #[serde(skip)]
    pub uploaded: bool,
}
impl VisualMemory {
    pub fn survey(terrain: &Terrain) -> Self {
        Self { cells: terrain.cells.clone(), rgba: terrain.rgba.clone(),
            objects: vec![Vec::new(); 15_000], previous_masks: Vec::new(), uploaded: false }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.cells.len() != CW * CH || self.rgba.len() != WW / 2 * crate::terrain::HH / 2 * 4
            || self.objects.len() != 15_000 {
            return Err("Invalid remembered terrain dimensions".into());
        }
        Ok(())
    }
    pub fn color(&self, page: usize) -> [u8; 4] {
        let x = page % 150 * 16 + 8; let y = page / 150 * 16 + 8;
        let i = (y * (WW / 2) + x) * 4;
        [self.rgba[i], self.rgba[i+1], self.rgba[i+2], 255]
    }
    /// Return contiguous changed page spans, never crossing a page row.
    pub fn refresh(&mut self, terrain: &Terrain, masks: &[u32], dirty: &[bool], live: &[Object]) -> Vec<(usize,usize)> {
        let first = !self.uploaded;
        let mut changed = vec![false; 15_000];
        for i in 0..15_000 {
            if masks[i] & VISIBLE == 0 { continue; }
            if first || dirty[i] || self.previous_masks.get(i).copied().unwrap_or(0) & VISIBLE == 0 {
                let x = i % 150; let y = i / 150;
                for row in 0..4 {
                    let start = (y*4+row)*CW+x*4;
                    self.cells[start..start+4].copy_from_slice(&terrain.cells[start..start+4]);
                }
                for row in 0..16 {
                    let start = ((y*16+row)*(WW/2)+x*16)*4;
                    self.rgba[start..start+64].copy_from_slice(&terrain.rgba[start..start+64]);
                }
                changed[i] = true;
            }
            // Rebuild only optically observed object fragments. Stale fragments
            // remain in other pages, including unseen demolition/construction.
            self.objects[i].clear();
        }
        for object in live {
            let lo = [((object.pos[0]-object.size*0.5)/32.).floor().max(0.) as usize,
                ((object.pos[1]-object.size*0.5)/32.).floor().max(0.) as usize];
            let hi = [((object.pos[0]+object.size*0.5)/32.).floor().clamp(0.,149.) as usize,
                ((object.pos[1]+object.size*0.5)/32.).floor().clamp(0.,99.) as usize];
            for y in lo[1]..=hi[1] { for x in lo[0]..=hi[0] {
                let i=y*150+x;
                if masks[i]&VISIBLE != 0 {
                    let mut fragment=*object;
                    fragment.kind=0x8000_0000 | i as u32;
                    self.objects[i].push(fragment);
                }
            }}
        }
        self.previous_masks=masks.to_vec();
        if first { changed.fill(true); }
        self.uploaded=true;
        let mut spans=Vec::new(); let mut i=0;
        while i<changed.len() {
            if !changed[i] {i+=1;continue;}
            let start=i; i+=1;
            while i<changed.len() && changed[i] && i/150==start/150 {i+=1;}
            spans.push((start,i));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_changes_wait_for_optical_observation_and_survive_roundtrip() {
        let mut terrain=Terrain::new(78421);
        let mut memory=VisualMemory::survey(&terrain);
        let original=memory.rgba[0];
        let mut masks=vec![0;15_000]; let dirty=vec![true;15_000];
        terrain.rgba[0]=original.wrapping_add(1);
        memory.refresh(&terrain,&masks,&dirty,&[]);
        assert_eq!(memory.rgba[0],original);
        masks[0]=crate::campaign_support::RADAR_SWEEP;
        memory.refresh(&terrain,&masks,&dirty,&[]);
        assert_eq!(memory.rgba[0],original);
        let saved=serde_json::to_vec(&memory).unwrap();
        let mut memory:VisualMemory=serde_json::from_slice(&saved).unwrap();
        assert_eq!(memory.rgba[0],original);
        masks[0]=VISIBLE;
        memory.refresh(&terrain,&masks,&dirty,&[]);
        assert_eq!(memory.rgba[0],terrain.rgba[0]);
    }
}
