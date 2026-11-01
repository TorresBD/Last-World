//! Bounded macro migration links derived from the authoritative navigation grid.
//! These masks describe available border crossings, not animal-owned positions.
use crate::tactics::{MovementClass, NavGrid};

pub fn macro_connections(
    grid: &NavGrid,
    width: usize,
    height: usize,
) -> Result<Vec<[u8; 2]>, String> {
    if width == 0 || height == 0 || grid.width % width != 0 || grid.height % height != 0 {
        return Err("Ecology/navigation dimensions do not align".into());
    }
    let sx = grid.width / width;
    let sy = grid.height / height;
    let mut masks = vec![[0u8; 2]; width * height];
    let classes = [
        MovementClass::Foot,
        MovementClass::Naval {
            draft_mm: 1,
            air_draft_mm: 0,
        },
    ];
    for py in 0..height {
        for px in 0..width {
            let patch = py * width + px;
            if px + 1 < width {
                let x = (px + 1) * sx;
                for (kind, class) in classes.iter().copied().enumerate() {
                    if (py * sy..(py + 1) * sy).any(|y| {
                        grid.adjacent_reachable(y * grid.width + x - 1, y * grid.width + x, class)
                    }) {
                        masks[patch][kind] |= 2;
                        masks[patch + 1][kind] |= 1;
                    }
                }
            }
            if py + 1 < height {
                let y = (py + 1) * sy;
                for (kind, class) in classes.iter().copied().enumerate() {
                    if (px * sx..(px + 1) * sx).any(|x| {
                        grid.adjacent_reachable((y - 1) * grid.width + x, y * grid.width + x, class)
                    }) {
                        masks[patch][kind] |= 8;
                        masks[patch + width][kind] |= 4;
                    }
                }
            }
        }
    }
    Ok(masks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactics::NavCell;
    #[test]
    fn continuous_wall_and_later_opening_change_both_sides() {
        let mut grid = NavGrid::new(4, 2, 16_000, vec![NavCell::default(); 8]).unwrap();
        for y in 0..2 {
            grid.set_cell(
                y * 4 + 2,
                NavCell {
                    blocked: true,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            macro_connections(&grid, 2, 1).unwrap(),
            vec![[0, 0], [0, 0]]
        );
        grid.set_cell(2, NavCell::default()).unwrap();
        assert_eq!(
            macro_connections(&grid, 2, 1).unwrap(),
            vec![[2, 0], [1, 0]]
        );
    }
    #[test]
    fn water_crossings_require_connected_depth_and_respect_reefs() {
        let mut grid = NavGrid::new(2, 1, 16_000, vec![NavCell::default(); 2]).unwrap();
        for i in 0..2 {
            grid.set_cell(
                i,
                NavCell {
                    water_depth_mm: 2000,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            macro_connections(&grid, 2, 1).unwrap(),
            vec![[0, 2], [0, 1]]
        );
        grid.replace_reefs(&[(1, 2000)]).unwrap();
        assert_eq!(
            macro_connections(&grid, 2, 1).unwrap(),
            vec![[0, 0], [0, 0]]
        );
    }
}
