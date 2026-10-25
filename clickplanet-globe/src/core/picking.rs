//! Screen → tile picking: the ray under the pointer is intersected with the unit sphere on the CPU
//! (see [`crate::core::controls::CameraFrame::pick_sphere`]), then the nearest tile centre is found
//! in an R*-tree of the ~258k tile positions.
//!
//! This was preferred over a GPU id buffer (what the original front end did with a picking render
//! pass and `readPixels`): reading back a pixel is asynchronous on WebGPU and stalls the pipeline on
//! WebGL2, and needs an extra render pass per pick. The CPU query is a few microseconds, synchronous,
//! identical on both backends, and cheap enough to run on every pointer move for hovering.

use glam::Vec3;
use rstar::primitives::GeomWithData;
use rstar::RTree;

type TilePoint = GeomWithData<[f32; 3], u32>;

pub struct TileIndex {
    tree: RTree<TilePoint>,
}

impl TileIndex {
    /// `positions` is the flat `[x, y, z, x, y, z, ...]` array of `coordinates.json`; tile ids are
    /// 1-based.
    pub fn new(positions: &[f32]) -> Self {
        #[allow(clippy::manual_slice_size_calculation, unknown_lints, clippy::chunks_exact_with_const_size)]
        let points = positions
            .chunks_exact(3)
            .enumerate()
            .map(|(i, p)| TilePoint::new([p[0], p[1], p[2]], i as u32 + 1))
            .collect();
        TileIndex {
            tree: RTree::bulk_load(points),
        }
    }

    /// The tile whose centre is closest to `point`, if it is closer than `max_distance`.
    pub fn nearest(&self, point: Vec3, max_distance: f32) -> Option<u32> {
        let query = [point.x, point.y, point.z];
        self.tree
            .nearest_neighbor(&query)
            .filter(|tile| {
                let [x, y, z] = *tile.geom();
                (Vec3::new(x, y, z) - point).length() <= max_distance
            })
            .map(|tile| tile.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_nearest_tile_within_range() {
        let positions = [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let index = TileIndex::new(&positions);
        assert_eq!(index.nearest(Vec3::new(0.99, 0.01, 0.0), 0.05), Some(2));
        assert_eq!(index.nearest(Vec3::new(0.0, 0.0, 1.0), 0.0), Some(1));
        assert_eq!(index.nearest(Vec3::new(0.5, 0.5, 0.0), 0.05), None);
    }
}
