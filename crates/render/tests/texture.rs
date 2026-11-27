//! The textured terrain's pure rules, with every expected number written
//! here as a literal worked from the clause (REN-093 to REN-105), never read
//! from a constant or a function of the crate.

use supercom_render::{
    Blend, Error, Filtering, LAYER_SLOTS, Layer, SAMPLED_TEXTURES, TerrainTextures, TextureFault,
    TextureItem, TextureLevel, blend_stored, check_textures, layer_coordinate, mask_coordinate,
    mask_weight, srgb_decode, textured_color,
};

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn near3(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
    a.iter().zip(b).all(|(a, b)| near(*a, b, tolerance))
}

/// Four layer colours (each unlike the others) in stored bytes: slot 0 to 8.
const LAYERS: [[u8; 4]; 9] = [
    [200, 40, 40, 9],
    [40, 200, 40, 9],
    [40, 40, 200, 9],
    [200, 200, 40, 9],
    [40, 200, 200, 9],
    [200, 40, 200, 9],
    [120, 120, 120, 9],
    [240, 160, 80, 9],
    [80, 160, 240, 9],
];

fn all_named() -> [Option<[u8; 4]>; 8] {
    [
        Some(LAYERS[1]),
        Some(LAYERS[2]),
        Some(LAYERS[3]),
        Some(LAYERS[4]),
        Some(LAYERS[5]),
        Some(LAYERS[6]),
        Some(LAYERS[7]),
        Some(LAYERS[8]),
    ]
}

fn top(alpha: u8) -> [u8; 4] {
    [10, 20, 250, alpha]
}

fn blended(
    blend: Blend,
    layers: [Option<[u8; 4]>; 8],
    alpha: u8,
    low: [u8; 4],
    high: [u8; 4],
) -> [f32; 3] {
    blend_stored(blend, LAYERS[0], layers, top(alpha), [low, high])
}

// REN-093
#[test]
fn render_mask_weight_is_clamped_twice_the_value_less_one() {
    // Worked from clamp(2m - 1, 0, 1) with m = byte / 255.
    for (byte, want) in [
        (0_u8, 0.0_f32),
        (64, 0.0),
        (127, 0.0),
        (128, 0.003_921_6),
        (191, 0.498_039_2),
        (255, 1.0),
    ] {
        assert!(
            near(mask_weight(byte), want, 1e-5),
            "the weight of {byte} is {}, want {want}",
            mask_weight(byte)
        );
    }
}

// REN-094
#[test]
fn render_blend_orders_layers_by_program_over_the_base() {
    let scale = 1.0 / 255.0;
    let t = |c: [f32; 3]| c.map(|v| v * scale);
    let cases: [(&str, [f32; 3], [f32; 3]); 7] = [
        // A first channel of 255 lays slot 1 over slot 0 whole.
        (
            "one channel",
            blended(Blend::Four, all_named(), 0, [255, 0, 0, 0], [0; 4]),
            t([40.0, 200.0, 40.0]),
        ),
        // All four channels at 255: each layer in turn, the last on top.
        (
            "order",
            blended(Blend::Four, all_named(), 0, [255; 4], [0; 4]),
            t([40.0, 200.0, 200.0]),
        ),
        // Distinct weights 127/255, 1, 1/255, 0 in the order of the channels.
        (
            "distinct weights",
            blended(Blend::Four, all_named(), 0, [191, 255, 128, 0], [0; 4]),
            [0.159_32, 0.159_32, 0.781_85],
        ),
        // The second program: slots 5 to 8 by the second mask's channels.
        (
            "eight layers",
            blended(Blend::Eight, all_named(), 0, [0; 4], [0, 191, 128, 0]),
            [0.629_29, 0.314_34, 0.313_11],
        ),
        // The first program ignores the second mask though it is all 255.
        (
            "four ignores the second mask",
            blended(Blend::Four, all_named(), 0, [0; 4], [255; 4]),
            t([200.0, 40.0, 40.0]),
        ),
        // An unnamed slot is skipped though its channel is 255.
        (
            "unnamed slot 1 skipped",
            {
                let mut layers = all_named();
                layers[0] = None;
                blended(Blend::Four, layers, 0, [255, 0, 0, 0], [0; 4])
            },
            t([200.0, 40.0, 40.0]),
        ),
        (
            "unnamed 5 to 8 skipped",
            blended(
                Blend::Eight,
                [
                    Some(LAYERS[1]),
                    Some(LAYERS[2]),
                    Some(LAYERS[3]),
                    Some(LAYERS[4]),
                    None,
                    None,
                    None,
                    None,
                ],
                0,
                [0; 4],
                [255; 4],
            ),
            t([200.0, 40.0, 40.0]),
        ),
    ];
    for (name, got, want) in cases {
        assert!(near3(got, want, 2e-4), "{name}: {got:?}, want {want:?}");
    }
}

// REN-095
#[test]
fn render_top_layer_is_laid_on_by_its_own_alpha() {
    let layers = all_named();
    let none = [0; 4];
    // Slot 0 alone (all weights 0), then slot 9 over it by alpha 0, 255, 100.
    for (alpha, want) in [
        (0_u8, [0.784_31_f32, 0.156_86, 0.156_86]),
        (255, [0.039_22, 0.078_43, 0.980_39]),
        (100, [0.492_12, 0.126_11, 0.479_82]),
    ] {
        let got = blended(Blend::Four, layers, alpha, none, none);
        assert!(
            near3(got, want, 2e-4),
            "alpha {alpha}: {got:?}, want {want:?}"
        );
    }
}

// REN-097
#[test]
fn render_mask_coordinate_runs_zero_to_one_across_the_map() {
    let extent = [64.0, 32.0];
    for ((x, z), want) in [
        ((0.0, 0.0), [0.0, 0.0]),
        ((64.0, 32.0), [1.0, 1.0]),
        ((32.0, 16.0), [0.5, 0.5]),
        ((16.0, 8.0), [0.25, 0.25]),
        // Past an edge the lookup clamps.
        ((-5.0, 40.0), [0.0, 1.0]),
        ((96.0, -1.0), [1.0, 0.0]),
    ] {
        let got = mask_coordinate(x, z, extent);
        assert!(
            near(got[0], want[0], 1e-6) && near(got[1], want[1], 1e-6),
            "({x}, {z}) maps to {got:?}, want {want:?}"
        );
    }
}

// REN-099
#[test]
fn render_layer_coordinate_repeats_by_world_over_scale() {
    for ((x, z, scale), want) in [
        ((0.0, 0.0, 4.0), [0.0, 0.0]),
        ((8.0, 12.0, 4.0), [2.0, 3.0]),
        // A map 64 wide holds 64 / 32 = 2 repeats across and 1 down at 32.
        ((64.0, 32.0, 32.0), [2.0, 1.0]),
        ((64.0, 64.0, 128.0), [0.5, 0.5]),
    ] {
        let got = layer_coordinate(x, z, scale);
        assert!(
            near(got[0], want[0], 1e-6) && near(got[1], want[1], 1e-6),
            "({x}, {z}) at scale {scale} maps to {got:?}, want {want:?}"
        );
    }
}

// REN-101
#[test]
fn render_textured_color_decodes_the_lit_stored_value() {
    // The transfer function at its breakpoint, a middle value, and the ends.
    for (v, want) in [
        (0.0_f32, 0.0_f32),
        (0.040_45, 0.003_130_8),
        (0.5, 0.214_041_1),
        (1.0, 1.0),
    ] {
        assert!(
            near(srgb_decode(v), want, 1e-6),
            "decode({v}) is {}, want {want}",
            srgb_decode(v)
        );
    }
    let stored = [128.0 / 255.0; 3];
    // Facing the light (-0.36, 0.8, -0.48): a factor of 1.
    let facing = textured_color(stored, [-0.36, 0.8, -0.48]);
    assert!(near3(facing, [0.215_860_5; 3], 1e-5), "{facing:?}");
    // Flat ground: n . L = 0.8, a factor of 0.3 + 0.7 * 0.8 = 0.86.
    let flat = textured_color(stored, [0.0, 1.0, 0.0]);
    assert!(near3(flat, [0.156_168; 3], 1e-5), "{flat:?}");
    // A normal turned away from the light holds the ambient share, 0.3.
    let away = textured_color([1.0; 3], [0.36, -0.8, 0.48]);
    assert!(near3(away, [0.073_238; 3], 1e-5), "{away:?}");
}

// ---- the checks of REN-104 ----------------------------------------------

fn level(width: u32, height: u32) -> TextureLevel {
    TextureLevel {
        width,
        height,
        rgba: vec![7; usize::try_from(width * height * 4).unwrap_or(0)],
    }
}

fn layer(sides: &[(u32, u32)]) -> Layer {
    Layer {
        levels: sides.iter().map(|&(w, h)| level(w, h)).collect(),
        scale: 4.0,
    }
}

fn good() -> TerrainTextures {
    TerrainTextures {
        blend: Blend::Eight,
        filtering: Filtering::Linear,
        base: layer(&[(4, 4), (2, 2), (1, 1)]),
        layers: std::array::from_fn(|_| Some(layer(&[(2, 2), (1, 1)]))),
        top: layer(&[(5, 3), (2, 1), (1, 1)]),
        low: level(4, 4),
        high: Some(level(4, 4)),
    }
}

fn refused(mut set: TerrainTextures, change: impl FnOnce(&mut TerrainTextures)) -> Error {
    change(&mut set);
    match check_textures(&set, 4096, 16) {
        Err(error) => error,
        Ok(()) => Error::NothingUploaded,
    }
}

fn fault(item: TextureItem, fault: TextureFault) -> Error {
    Error::Texture { item, fault }
}

// REN-104
#[test]
fn render_texture_checks_refuse_each_fault() {
    assert_eq!(LAYER_SLOTS, 10);
    assert_eq!(check_textures(&good(), 4096, 16), Ok(()));
    // A chain that halves rounding down and never below 1 passes: 5 by 3,
    // 2 by 1, 1 by 1.
    assert_eq!(
        refused(good(), |s| s.base.levels[0].rgba.pop().map_or((), drop)),
        fault(
            TextureItem::Layer(0),
            TextureFault::Bytes {
                level: 0,
                expected: 64,
                found: 63
            }
        ),
        "a byte short of width times height times 4"
    );
    assert_eq!(
        refused(good(), |s| s.low.rgba.extend([0; 4])),
        fault(
            TextureItem::Mask(0),
            TextureFault::Bytes {
                level: 0,
                expected: 64,
                found: 68
            }
        )
    );
    assert_eq!(
        refused(good(), |s| s.top.levels[0].width = 0),
        fault(TextureItem::Layer(9), TextureFault::ZeroSide { level: 0 })
    );
    assert_eq!(
        refused(good(), |s| {
            if let Some(l) = s.layers[2].as_mut() {
                l.levels[1].height = 0;
            }
        }),
        fault(TextureItem::Layer(3), TextureFault::ZeroSide { level: 1 })
    );
    assert_eq!(
        check_textures(&good(), 3, 16),
        Err(fault(
            TextureItem::Layer(0),
            TextureFault::Oversize {
                level: 0,
                side: 4,
                limit: 3
            }
        )),
        "a side above the device's largest"
    );
    assert_eq!(
        refused(good(), |s| s.base.levels[1] = level(1, 2)),
        fault(TextureItem::Layer(0), TextureFault::Chain { level: 1 }),
        "a level that is not the one before halved"
    );
    assert_eq!(
        refused(good(), |s| {
            s.base.levels = vec![level(2, 2), level(1, 1), level(1, 1)];
        }),
        fault(TextureItem::Layer(0), TextureFault::Chain { level: 2 }),
        "no more levels than the larger side has: 2 by 2 has two"
    );
    assert_eq!(
        refused(good(), |s| s.top.levels[1] = level(3, 1)),
        fault(TextureItem::Layer(9), TextureFault::Chain { level: 1 })
    );
    assert_eq!(
        refused(good(), |s| s.base.levels.clear()),
        fault(TextureItem::Layer(0), TextureFault::NoLevels)
    );
    for scale in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
        let got = refused(good(), |s| {
            if let Some(l) = s.layers[4].as_mut() {
                l.scale = scale;
            }
        });
        assert!(
            matches!(
                got,
                Error::Texture {
                    item: TextureItem::Layer(5),
                    fault: TextureFault::Scale { .. }
                }
            ),
            "scale {scale}: {got:?}"
        );
    }
    assert_eq!(
        refused(good(), |s| s.high = None),
        fault(TextureItem::Mask(1), TextureFault::Missing),
        "the second program reads the second mask"
    );
    // The first program does not: no second mask is no fault.
    let mut four = good();
    four.blend = Blend::Four;
    four.high = None;
    assert_eq!(check_textures(&four, 4096, 16), Ok(()));
    // An unnamed layer is no fault.
    let mut sparse = good();
    sparse.layers[0] = None;
    assert_eq!(check_textures(&sparse, 4096, 16), Ok(()));
}

// REN-104
#[test]
fn render_texture_checks_refuse_a_device_with_too_few_sampled_textures() {
    assert_eq!(SAMPLED_TEXTURES, 12);
    assert_eq!(
        check_textures(&good(), 4096, 11),
        Err(Error::TextureBinding {
            needed: 12,
            limit: 11
        })
    );
    assert_eq!(check_textures(&good(), 4096, 12), Ok(()));
}
