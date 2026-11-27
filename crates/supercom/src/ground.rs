//! The map's ground textures: which of the map's files the terrain's program
//! draws, their reading and decoding, the decision between the textured
//! terrain and the height ramp, and the hand-over to the renderer.

use supercom_render::{Blend, Filtering, Renderer, TerrainTextures};
use supercom_scmap::Map;

/// What an item of the terrain's set is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    /// The program's name.
    Program,
    /// The layer of this slot, 0 to 9.
    Layer(usize),
    /// The mask of this index, 0 for the first.
    Mask(usize),
    /// The set as a whole, which the renderer refused.
    Set,
}

/// What reading and decoding an item came to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    /// The map names no file for the slot.
    Unnamed,
    /// Read, decoded, and checked.
    Ready,
    /// Missing, unreadable, or refused, with the reason.
    Failed(String),
}

/// One item the program draws, with the path the map stores for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checked {
    /// Which item.
    pub item: Item,
    /// The path as the map stores it, empty when none is named.
    pub path: String,
    /// What came of it.
    pub status: Status,
}

/// Why the terrain is drawn by the height ramp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice {
    /// The first failing item.
    pub item: Item,
    /// Its path as the map stores it.
    pub path: String,
    /// Why it failed.
    pub reason: String,
    /// How many items failed.
    pub failed: usize,
}

/// The decision between the textured terrain and the fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The textured terrain, with this blend.
    Textured(Blend),
    /// The height ramp, for this reason.
    Fallback(Notice),
}

/// What [`resolve`] did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolution {
    /// The decision, which the renderer now follows.
    pub verdict: Verdict,
    /// Files read through the reader.
    pub files_read: usize,
    /// Bytes of every decoded level of the layers.
    pub texture_bytes: u64,
    /// Bytes of every decoded mask.
    pub mask_bytes: u64,
}

/// The blend a program name keys, or `None` for a name that is none of the
/// three.
#[must_use]
pub fn blend_for(program: &str) -> Option<Blend> {
    let _ = program;
    None
}

/// A path as the archive reader takes it.
#[must_use]
pub fn normalize(path: &str) -> String {
    path.to_owned()
}

/// Decides from the program's name and what each item came to.
#[must_use]
pub fn decide(program: &str, items: &[Checked]) -> Verdict {
    let _ = (program, items);
    Verdict::Textured(Blend::Four)
}

/// What [`gather`] read.
pub struct Gathered {
    /// The program's name, then the items the program draws, in the order of
    /// the notice.
    pub items: Vec<Checked>,
    /// The set, when every item is ready.
    pub textures: Option<TerrainTextures>,
    /// Files read.
    pub files_read: usize,
    /// Bytes of every decoded level of the layers.
    pub texture_bytes: u64,
    /// Bytes of every decoded mask.
    pub mask_bytes: u64,
}

/// Reads and decodes what `map`'s program draws.
pub fn gather(
    map: &Map,
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    filtering: Filtering,
) -> Gathered {
    let _ = (map, read, filtering);
    Gathered {
        items: Vec::new(),
        textures: None,
        files_read: 0,
        texture_bytes: 0,
        mask_bytes: 0,
    }
}

/// Reads, decodes, decides, and hands the renderer the set or nothing.
pub fn resolve(
    map: &Map,
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    renderer: &mut Renderer,
    filtering: Filtering,
) -> Resolution {
    let _ = (map, read, renderer, filtering);
    Resolution {
        verdict: Verdict::Textured(Blend::Four),
        files_read: 0,
        texture_bytes: 0,
        mask_bytes: 0,
    }
}

impl std::fmt::Display for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = f;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
