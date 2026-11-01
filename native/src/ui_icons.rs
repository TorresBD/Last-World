//! Authored pixel maps; rendering copies these exact palette pixels.
use std::{collections::BTreeMap,sync::OnceLock};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct PixelIcon { pub small:Vec<String>, pub large:Vec<String> }
#[derive(Deserialize)]
struct Catalog { palette:BTreeMap<String,Option<[u8;4]>>,icons:BTreeMap<String,PixelIcon> }
fn catalog()->&'static Catalog {
    static DATA:OnceLock<Catalog>=OnceLock::new();
    DATA.get_or_init(||serde_json::from_str(include_str!("../../assets/icons/manifest.json")).expect("validated pixel-map catalog"))
}
pub fn get(key:&str)->Option<&'static PixelIcon> {
    let key=if key.starts_with("unit:0:0:"){ "builder" }
        else if key.starts_with("unit:0:1:"){ "defender" }
        else if key.starts_with("unit:0:2:"){ "scout" }
        else {match key {"rally"=>"move","restore"=>"smooth","boat"=>"gunboat",_=>key}};
    catalog().icons.get(key)
}
pub fn color(symbol:u8)->Option<[u8;4]> {
    catalog().palette.get(&(symbol as char).to_string()).copied().flatten()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_command_has_a_distinct_valid_pixel_map() {
        let mut seen=std::collections::BTreeSet::new();
        for i in 0..33 {
            let icon=get(crate::ui_interaction::tool_icon(i)).expect("command icon");
            assert!(seen.insert(icon.small.join("")),"duplicate command icon {i}");
            for (rows,size) in [(&icon.small,16),(&icon.large,32)] {
                assert_eq!(rows.len(),size);
                for row in rows { assert_eq!(row.len(),size); for s in row.bytes(){
                    assert!(catalog().palette.contains_key(&(s as char).to_string()));
                }}
            }
        }
    }
}
