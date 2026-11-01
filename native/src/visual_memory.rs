//! Sparse last-observed 32m pages, separate from authoritative simulation.
//! Initial fine survey is regenerated into the existing renderer on load.
//! CPU keeps only a coarse survey and fine pages that were optically observed.
use crate::{terrain::{Terrain, Cell, CW, CH, WW}, gpu::Object, campaign_support::VISIBLE};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
#[derive(Clone, Serialize, Deserialize)]
pub struct MemoryPage {pub cells:Vec<Cell>,pub rgba:Vec<u8>}
#[derive(Clone, Serialize, Deserialize)]
pub struct VisualMemory {
    pub survey_cells:Vec<Cell>,
    pub survey_colors:Vec<[u8;4]>,
    pub pages:BTreeMap<usize,MemoryPage>,
    pub objects:Vec<Vec<Object>>,
    #[serde(skip)] pub previous_masks:Vec<u32>,
    #[serde(skip)] pub uploaded:bool,
}
impl VisualMemory {
    pub fn survey(terrain:&Terrain)->Self {
        let mut cells=Vec::with_capacity(15_000);
        let mut colors=Vec::with_capacity(15_000);
        for page in 0..15_000 {
            cells.push(terrain.cells[(page/150*4+2)*CW+page%150*4+2]);
            let pixel=((page/150*16+8)*(WW/2)+page%150*16+8)*4;
            colors.push([terrain.rgba[pixel],terrain.rgba[pixel+1],terrain.rgba[pixel+2],255]);
        }
        Self{survey_cells:cells,survey_colors:colors,pages:BTreeMap::new(),
            objects:vec![Vec::new();15_000],previous_masks:Vec::new(),uploaded:false}
    }
    pub fn validate(&self)->Result<(),String>{
        if self.survey_cells.len()!=15_000||self.survey_colors.len()!=15_000||self.objects.len()!=15_000||
            self.pages.iter().any(|(&id,page)|id>=15_000||page.cells.len()!=16||page.rgba.len()!=1024) {
            return Err("Invalid sparse remembered terrain".into());
        }
        Ok(())
    }
    pub fn cell_at(&self,cell:usize)->Cell {
        let page=(cell/CW/4)*150+(cell%CW/4);
        self.pages.get(&page).map(|p|p.cells[(cell/CW%4)*4+cell%4]).unwrap_or(self.survey_cells[page])
    }
    pub fn color(&self,page:usize)->[u8;4]{
        if let Some(p)=self.pages.get(&page){let i=(8*16+8)*4;[p.rgba[i],p.rgba[i+1],p.rgba[i+2],255]}
        else{self.survey_colors[page]}
    }
    pub fn refresh(&mut self,terrain:&Terrain,masks:&[u32],dirty:&[bool],live:&[Object])->Vec<(usize,usize)>{
        let first=!self.uploaded;
        let mut changed=vec![false;15_000];
        if first {for &page in self.pages.keys(){changed[page]=true;}}
        for i in 0..15_000 {
            if masks[i]&VISIBLE==0{continue;}
            if first||dirty[i]||self.previous_masks.get(i).copied().unwrap_or(0)&VISIBLE==0 {
                let x=i%150;let y=i/150;
                let mut cells=Vec::with_capacity(16);let mut rgba=Vec::with_capacity(1024);
                for row in 0..4{let start=(y*4+row)*CW+x*4;cells.extend_from_slice(&terrain.cells[start..start+4]);}
                for row in 0..16{let start=((y*16+row)*(WW/2)+x*16)*4;rgba.extend_from_slice(&terrain.rgba[start..start+64]);}
                self.pages.insert(i,MemoryPage{cells,rgba});changed[i]=true;
            }
            self.objects[i].clear();
        }
        for object in live {
            let lo=[((object.pos[0]-object.size*0.5)/32.).floor().max(0.) as usize,
                ((object.pos[1]-object.size*0.5)/32.).floor().max(0.) as usize];
            let hi=[((object.pos[0]+object.size*0.5)/32.).floor().clamp(0.,149.) as usize,
                ((object.pos[1]+object.size*0.5)/32.).floor().clamp(0.,99.) as usize];
            for y in lo[1]..=hi[1]{for x in lo[0]..=hi[0]{
                let i=y*150+x;if masks[i]&VISIBLE!=0{let mut fragment=*object;fragment.kind=0x8000_0000|i as u32;self.objects[i].push(fragment);}
            }}
        }
        self.previous_masks=masks.to_vec();self.uploaded=true;
        let mut spans=Vec::new();let mut i=0;
        while i<changed.len(){if !changed[i]{i+=1;continue;}let start=i;i+=1;
            while i<changed.len()&&changed[i]&&i/150==start/150{i+=1;}spans.push((start,i));}
        spans
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn hidden_changes_wait_for_optical_observation_and_survive_roundtrip(){
        let mut terrain=Terrain::new(78421);let mut memory=VisualMemory::survey(&terrain);
        assert!(memory.pages.is_empty());
        let original=memory.color(0);
        let pixel=(8*(WW/2)+8)*4;terrain.rgba[pixel]=original[0].wrapping_add(1);
        let mut masks=vec![0;15_000];let dirty=vec![true;15_000];
        memory.refresh(&terrain,&masks,&dirty,&[]);assert_eq!(memory.color(0),original);
        masks[0]=crate::campaign_support::RADAR_SWEEP;
        memory.refresh(&terrain,&masks,&dirty,&[]);assert_eq!(memory.color(0),original);assert!(memory.pages.is_empty());
        masks[0]=VISIBLE;memory.refresh(&terrain,&masks,&dirty,&[]);
        let seen=memory.color(0);assert_eq!(seen[0],terrain.rgba[pixel]);
        masks[0]=0;terrain.rgba[pixel]=original[0];memory.refresh(&terrain,&masks,&dirty,&[]);
        let mut restored:VisualMemory=serde_json::from_slice(&serde_json::to_vec(&memory).unwrap()).unwrap();
        assert_eq!(restored.color(0),seen);
        masks[0]=VISIBLE;restored.refresh(&terrain,&masks,&dirty,&[]);assert_eq!(restored.color(0),original);
    }
    #[test]
    fn unseen_demolition_remains_as_optically_recorded_fragments(){
        let terrain=Terrain::new(78421);let mut memory=VisualMemory::survey(&terrain);
        let mut masks=vec![0;15_000];masks[0]=VISIBLE;
        let object=Object{pos:[16.,16.],size:8.,kind:0,uv:[0,0,16,16],team:0,height:2.,complete:1.,seed:1};
        memory.refresh(&terrain,&masks,&vec![true;15_000],&[object]);assert_eq!(memory.objects[0].len(),1);
        masks[0]=crate::campaign_support::RADAR_COVERAGE;
        memory.refresh(&terrain,&masks,&vec![true;15_000],&[]);assert_eq!(memory.objects[0].len(),1);
        masks[0]=VISIBLE;memory.refresh(&terrain,&masks,&vec![true;15_000],&[]);assert!(memory.objects[0].is_empty());
    }
}
