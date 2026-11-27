//! The textured terrain's plain inputs and the pure rules that decide what a
//! fragment holds: the weight of a mask value, the blend of a program, the
//! coordinates of a lookup, the colour rule, and the checks made before any
//! texture is created. Nothing here names a loader type.

use crate::Error;

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

/// The weight of a mask value.
#[must_use]
pub fn mask_weight(value: u8) -> f32 {
    let _ = value;
    0.0
}

/// A layer texel as four stored bytes, or `None` for an unnamed slot.
pub type Texel = Option<[u8; 4]>;

/// The blended colour of a fragment in stored numbers, each channel 0 to 1.
#[must_use]
pub fn blend_stored(
    blend: Blend,
    base: [u8; 4],
    layers: [Texel; 8],
    top: [u8; 4],
    masks: [[u8; 4]; 2],
) -> [f32; 3] {
    let _ = (blend, layers, top, masks);
    [f32::from(base[0]), 0.0, 0.0]
}

/// The mask coordinate of the world point `(x, z)` on a map of `extent`.
#[must_use]
pub fn mask_coordinate(x: f32, z: f32, extent: [f32; 2]) -> [f32; 2] {
    let _ = (x, z, extent);
    [0.0, 0.0]
}

/// The coordinate of the world point `(x, z)` in a layer of `scale`.
#[must_use]
pub fn layer_coordinate(x: f32, z: f32, scale: f32) -> [f32; 2] {
    let _ = (x, z, scale);
    [0.0, 0.0]
}

/// The sRGB transfer function's decoding of a stored value.
#[must_use]
pub fn srgb_decode(value: f32) -> f32 {
    value
}

/// The linear colour a fragment returns for a blended stored colour and a
/// surface normal.
#[must_use]
pub fn textured_color(stored: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let _ = normal;
    stored
}

/// Checks `textures` against a device whose largest 2D side is `max_side`
/// and which allows `max_sampled` sampled textures a shader stage.
///
/// # Errors
/// [`Error::Texture`] and [`Error::TextureBinding`].
pub fn check_textures(
    textures: &TerrainTextures,
    max_side: u32,
    max_sampled: u32,
) -> Result<(), Error> {
    let _ = (textures, max_side, max_sampled);
    Ok(())
}
