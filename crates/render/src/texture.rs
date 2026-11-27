//! The textured terrain's plain inputs and the pure rules that decide what a
//! fragment holds: the weight of a mask value, the blend of a program, the
//! coordinates of a lookup, the colour rule, and the checks made before any
//! texture is created. Nothing here names a loader type.

use crate::{AMBIENT, Error, LIGHT_DIRECTION, TextureFault, TextureItem};

/// Terrain layers a map names: slot 0 is the base, slots 1 to 8 are blended
/// over it, slot 9 is laid on by its own alpha.
pub const LAYER_SLOTS: usize = 10;

/// Sampled textures the textured terrain's pass binds: ten layers and two
/// masks, an unnamed layer and an unread mask bound to a one-texel stand-in.
pub const SAMPLED_TEXTURES: u32 = 12;

/// One level of a texture: `width` by `height` pixels of four bytes each, in
/// the order red, green, blue, alpha, the first stored row first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureLevel {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// A terrain layer: its levels, the largest first, and the scale the map
/// stores for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// The levels, each halving the one before, rounding down and never
    /// below 1.
    pub levels: Vec<TextureLevel>,
    /// World units one repeat of the layer covers on each axis.
    pub scale: f32,
}

/// Which layers a program blends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Blend {
    /// Slots 1 to 4 over slot 0, weighed by the first mask, then slot 9.
    Four,
    /// Slots 5 to 8 as well, weighed by the second mask, before slot 9.
    Eight,
}

/// How the lookups filter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Filtering {
    /// One stored texel, nearest, at the level chosen.
    Nearest,
    /// Linear within a level and between levels.
    Linear,
}

/// Everything the textured terrain draws from, as plain buffers.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTextures {
    /// The program's blend.
    pub blend: Blend,
    /// The lookups' filtering.
    pub filtering: Filtering,
    /// Slot 0.
    pub base: Layer,
    /// Slots 1 to 8; `None` is an unnamed slot, which is skipped.
    pub layers: [Option<Layer>; 8],
    /// Slot 9.
    pub top: Layer,
    /// The first mask.
    pub low: TextureLevel,
    /// The second mask; read only by [`Blend::Eight`].
    pub high: Option<TextureLevel>,
}

/// The weight of a mask value: `clamp(2m - 1, 0, 1)` for the stored byte's
/// value `m` from 0 to 1.
#[must_use]
pub fn mask_weight(value: u8) -> f32 {
    (2.0 * f32::from(value) / 255.0 - 1.0).clamp(0.0, 1.0)
}

/// A layer texel as four stored bytes, or `None` for an unnamed slot.
pub type Texel = Option<[u8; 4]>;

fn stored(texel: [u8; 4]) -> [f32; 3] {
    [0, 1, 2].map(|k| f32::from(texel[k]) / 255.0)
}

fn mix(from: [f32; 3], to: [f32; 3], weight: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| from[k] + (to[k] - from[k]) * weight)
}

/// The blended colour of a fragment in stored numbers, each channel 0 to 1:
/// `base` with `layers` (slots 1 to 8, `None` when unnamed) blended over it
/// in turn by the weights of `masks` (the first mask's channels for slots 1
/// to 4, the second's for slots 5 to 8, which only [`Blend::Eight`] blends),
/// then `top` laid on by its own alpha. Each mask is a texel's four stored
/// bytes.
#[must_use]
pub fn blend_stored(
    blend: Blend,
    base: [u8; 4],
    layers: [Texel; 8],
    top: [u8; 4],
    masks: [[u8; 4]; 2],
) -> [f32; 3] {
    let mut color = stored(base);
    let blended = match blend {
        Blend::Four => 4,
        Blend::Eight => 8,
    };
    for (k, layer) in layers.iter().take(blended).enumerate() {
        if let Some(layer) = layer {
            color = mix(color, stored(*layer), mask_weight(masks[k / 4][k % 4]));
        }
    }
    mix(color, stored(top), f32::from(top[3]) / 255.0)
}

/// The mask coordinate of the world point `(x, z)` on a map of `extent`:
/// `(x / width, z / height)`, clamped to 0 to 1.
#[must_use]
pub fn mask_coordinate(x: f32, z: f32, extent: [f32; 2]) -> [f32; 2] {
    [
        (x / extent[0]).clamp(0.0, 1.0),
        (z / extent[1]).clamp(0.0, 1.0),
    ]
}

/// The coordinate of the world point `(x, z)` in a layer whose repeat covers
/// `scale` world units: `(x / scale, z / scale)`, unbounded because the
/// layer repeats.
#[must_use]
pub fn layer_coordinate(x: f32, z: f32, scale: f32) -> [f32; 2] {
    [x / scale, z / scale]
}

/// The sRGB transfer function's decoding of a stored value: `v / 12.92` at
/// or below 0.04045, otherwise `((v + 0.055) / 1.055)` to the power 2.4.
#[must_use]
pub fn srgb_decode(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// The linear colour a fragment returns for a blended stored colour and a
/// surface normal: the colour times the first slice's light, `AMBIENT + (1 -
/// AMBIENT) * max(n . L, 0)` with `normal` renormalized, clamped to 0 to 1,
/// then decoded by [`srgb_decode`].
#[must_use]
pub fn textured_color(stored: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let length = normal.iter().map(|c| c * c).sum::<f32>().sqrt();
    let facing = if length > 0.0 {
        (normal
            .iter()
            .zip(LIGHT_DIRECTION)
            .map(|(n, l)| n / length * l)
            .sum::<f32>())
        .max(0.0)
    } else {
        0.0
    };
    let light = AMBIENT + (1.0 - AMBIENT) * facing;
    stored.map(|c| srgb_decode((c * light).clamp(0.0, 1.0)))
}

fn refuse<T>(item: TextureItem, fault: TextureFault) -> Result<T, Error> {
    Err(Error::Texture { item, fault })
}

/// Checks the levels of one texture: each with two positive sides within the
/// device's, exactly width times height times 4 bytes, and each the one
/// before halved, rounding down and never below 1, with no more levels than
/// the first level's larger side has.
fn check_levels(item: TextureItem, levels: &[TextureLevel], max_side: u32) -> Result<(), Error> {
    let Some(first) = levels.first() else {
        return refuse(item, TextureFault::NoLevels);
    };
    let most = 32 - first.width.max(first.height).leading_zeros();
    let mut previous: Option<(u32, u32)> = None;
    for (level, texture) in levels.iter().enumerate() {
        let (width, height) = (texture.width, texture.height);
        if width == 0 || height == 0 {
            return refuse(item, TextureFault::ZeroSide { level });
        }
        let side = width.max(height);
        if side > max_side {
            return refuse(
                item,
                TextureFault::Oversize {
                    level,
                    side,
                    limit: max_side,
                },
            );
        }
        let expected = u64::from(width) * u64::from(height) * 4;
        if u64::try_from(texture.rgba.len()) != Ok(expected) {
            return refuse(
                item,
                TextureFault::Bytes {
                    level,
                    expected,
                    found: texture.rgba.len(),
                },
            );
        }
        let halved = previous.map(|(w, h)| ((w / 2).max(1), (h / 2).max(1)));
        let beyond = u32::try_from(level).map_or(true, |at| at >= most);
        if beyond || halved.is_some_and(|halved| halved != (width, height)) {
            return refuse(item, TextureFault::Chain { level });
        }
        previous = Some((width, height));
    }
    Ok(())
}

fn check_layer(slot: usize, layer: &Layer, max_side: u32) -> Result<(), Error> {
    let item = TextureItem::Layer(slot);
    check_levels(item, &layer.levels, max_side)?;
    if layer.scale.is_finite() && layer.scale > 0.0 {
        Ok(())
    } else {
        refuse(item, TextureFault::Scale { scale: layer.scale })
    }
}

/// Checks `textures` against a device whose largest 2D side is `max_side`
/// and which allows `max_sampled` sampled textures a shader stage, before
/// anything is created: the binding count first, then slot 0, slots 1 to 8,
/// slot 9, the first mask, and the second mask, the second program's second
/// mask being required.
///
/// # Errors
/// [`Error::TextureBinding`] when the pass binds more sampled textures than
/// the device allows, and [`Error::Texture`] for the first texture that has
/// no level, a level with a zero side, a side above the device's, the wrong
/// byte count, or a break in the chain, a layer whose scale is not finite
/// and positive, or the second program's missing second mask.
pub fn check_textures(
    textures: &TerrainTextures,
    max_side: u32,
    max_sampled: u32,
) -> Result<(), Error> {
    if SAMPLED_TEXTURES > max_sampled {
        return Err(Error::TextureBinding {
            needed: SAMPLED_TEXTURES,
            limit: max_sampled,
        });
    }
    check_layer(0, &textures.base, max_side)?;
    for (k, layer) in textures.layers.iter().enumerate() {
        if let Some(layer) = layer {
            check_layer(k + 1, layer, max_side)?;
        }
    }
    check_layer(9, &textures.top, max_side)?;
    check_levels(
        TextureItem::Mask(0),
        std::slice::from_ref(&textures.low),
        max_side,
    )?;
    match (&textures.high, textures.blend) {
        (Some(high), _) => check_levels(TextureItem::Mask(1), std::slice::from_ref(high), max_side),
        (None, Blend::Eight) => refuse(TextureItem::Mask(1), TextureFault::Missing),
        (None, Blend::Four) => Ok(()),
    }
}
