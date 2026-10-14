//! The map digest of a wet map (docs/specs/sim.md, SIM-049 to SIM-051):
//! the water surface's value is part of the content a replay checks, so
//! two maps that differ only in the surface differ in digest. The expected
//! bytes are an independent encoding of the layout documented on
//! `map_digest`. All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use sha2::{Digest as _, Sha256};
use supercom_sim::{MapGrid, map_digest};

fn wet(surface: f32) -> MapGrid {
    // A 2 by 2 map: a 3 by 3 grid of samples in raw units of 2^-7.
    let samples: [i16; 9] = [0, 10, 20, 30, 40, 50, 60, 70, 80];
    MapGrid::from_heightmap_with_water(2, 2, &samples, 0.007_812_5, Some(surface))
        .expect("a wet map")
}

/// The documented layout: `map-v1` as a length-prefixed string, the map
/// name the same, width and height as little-endian `u32`, every height as
/// the little-endian raw `i64` bits, the surface as an option (0, or 1 and
/// the raw bits), and the marker count.
fn encoded(name: &str, map: &MapGrid) -> [u8; 32] {
    let mut h = Sha256::new();
    for s in ["map-v1", name] {
        h.update(u32::try_from(s.len()).expect("short").to_le_bytes());
        h.update(s.as_bytes());
    }
    h.update(map.width.to_le_bytes());
    h.update(map.height.to_le_bytes());
    for v in &map.heights {
        h.update(v.to_bits().to_le_bytes());
    }
    match map.water {
        None => h.update([0_u8]),
        Some(surface) => {
            h.update([1_u8]);
            h.update(surface.to_bits().to_le_bytes());
        }
    }
    h.update(0_u32.to_le_bytes());
    h.finalize().into()
}

#[test]
fn sim_map_digest_hashes_the_surface_value() {
    let (a, b) = (wet(1.0), wet(2.0));
    assert_eq!(a.heights, b.heights, "the maps differ only in the surface");
    assert_eq!(map_digest("m", &a, &[]), encoded("m", &a));
    assert_eq!(map_digest("m", &b, &[]), encoded("m", &b));
    assert_ne!(
        map_digest("m", &a, &[]),
        map_digest("m", &b, &[]),
        "the surface value is in the digest"
    );
}
