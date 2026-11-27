//! The map's ground textures: which of the map's files the terrain's program
//! draws, their reading and decoding, the decision between the textured
//! terrain and the height ramp, and the hand-over to the renderer.

use crate::error::RunError;
use std::fmt;
use supercom_render::{
    Blend, Error as RenderError, Filtering, Layer, Renderer, TerrainTextures, TextureItem,
    TextureLevel, grid_fits,
};
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

/// The reason the first case of the fallback gives.
const PROGRAM_REASON: &str = "the program is none of TTerrain, TTerrainXP, and TTerrainGlow";

/// The blend a program name keys, or `None` for a name that is none of the
/// three. `TTerrainGlow` is drawn by `TTerrain`'s rule.
#[must_use]
pub fn blend_for(program: &str) -> Option<Blend> {
    match program {
        "TTerrain" | "TTerrainGlow" => Some(Blend::Four),
        "TTerrainXP" => Some(Blend::Eight),
        _ => None,
    }
}

/// A path as the archive reader takes it: one leading slash stripped, each
/// backslash a slash, lowercased (FMT-050).
#[must_use]
pub fn normalize(path: &str) -> String {
    path.strip_prefix('/')
        .unwrap_or(path)
        .replace('\\', "/")
        .to_lowercase()
}

/// The items a program reads, in the order of the notice: slots 0 to 4 and 9
/// and the first mask, and under the second program slots 5 to 8 and the
/// second mask too.
fn needed(blend: Blend) -> Vec<Item> {
    let layers: &[usize] = match blend {
        Blend::Four => &[0, 1, 2, 3, 4, 9],
        Blend::Eight => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    };
    let masks: &[usize] = match blend {
        Blend::Four => &[0],
        Blend::Eight => &[0, 1],
    };
    layers
        .iter()
        .map(|slot| Item::Layer(*slot))
        .chain(masks.iter().map(|at| Item::Mask(*at)))
        .collect()
}

/// Decides from the program's name and what each item came to: the textured
/// terrain when every item the program reads is ready (a slot from 1 to 8
/// the map leaves unnamed is skipped), otherwise the height ramp, with the
/// first failing item in the order program, slot 0 to slot 9, first mask,
/// second mask, and the count of the items that failed.
#[must_use]
pub fn decide(program: &str, items: &[Checked]) -> Verdict {
    let Some(blend) = blend_for(program) else {
        return Verdict::Fallback(Notice {
            item: Item::Program,
            path: program.to_owned(),
            reason: PROGRAM_REASON.to_owned(),
            failed: 1,
        });
    };
    let mut failures = Vec::new();
    for item in needed(blend) {
        let found = items.iter().find(|checked| checked.item == item);
        let reason = match found.map(|checked| &checked.status) {
            Some(Status::Ready) => None,
            Some(Status::Unnamed) => match item {
                Item::Layer(0 | 9) => Some("the map names no file for the slot".to_owned()),
                _ => None,
            },
            Some(Status::Failed(reason)) => Some(reason.clone()),
            None => Some("not read".to_owned()),
        };
        if let Some(reason) = reason {
            let path = found
                .map(|checked| checked.path.clone())
                .unwrap_or_default();
            failures.push((item, path, reason));
        }
    }
    let failed = failures.len();
    match failures.into_iter().next() {
        None => Verdict::Textured(blend),
        Some((item, path, reason)) => Verdict::Fallback(Notice {
            item,
            path,
            reason,
            failed,
        }),
    }
}

/// What [`gather`] read.
pub struct Gathered {
    /// The items the program reads, in the order of the notice.
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

fn levels_of(decoded: Vec<supercom_dds::Level>) -> Vec<TextureLevel> {
    decoded
        .into_iter()
        .map(|level| TextureLevel {
            width: level.width,
            height: level.height,
            rgba: level.rgba,
        })
        .collect()
}

fn bytes_of(levels: &[TextureLevel]) -> u64 {
    levels
        .iter()
        .map(|level| u64::try_from(level.rgba.len()).unwrap_or(u64::MAX))
        .sum()
}

/// Reads and decodes what `map`'s program draws and nothing else: each named
/// layer's file through `read`, the paths normalized, and the masks from the
/// map's own bytes. A program that is none of the three reads nothing.
pub fn gather(
    map: &Map,
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    filtering: Filtering,
) -> Gathered {
    let mut got = Gathered {
        items: Vec::new(),
        textures: None,
        files_read: 0,
        texture_bytes: 0,
        mask_bytes: 0,
    };
    let Some(blend) = blend_for(&map.terrain_program) else {
        return got;
    };
    let mut layers: [Option<Layer>; 10] = std::array::from_fn(|_| None);
    for item in needed(blend) {
        let Item::Layer(slot) = item else { continue };
        let stratum = map.strata.get(slot);
        let path = stratum.map(|s| s.texture.clone()).unwrap_or_default();
        let status = if path.is_empty() {
            Status::Unnamed
        } else {
            got.files_read += 1;
            match read(&normalize(&path)) {
                Err(reason) => Status::Failed(format!("cannot read: {reason}")),
                Ok(bytes) => match supercom_dds::decode(&bytes) {
                    Err(error) => Status::Failed(format!("cannot decode: {error}")),
                    Ok(decoded) => {
                        let levels = levels_of(decoded);
                        got.texture_bytes += bytes_of(&levels);
                        let scale = stratum.map_or(f32::NAN, |s| s.scale);
                        if scale.is_finite() && scale > 0.0 {
                            layers[slot] = Some(Layer { levels, scale });
                            Status::Ready
                        } else {
                            Status::Failed(format!("the scale {scale} is not finite and positive"))
                        }
                    }
                },
            }
        };
        got.items.push(Checked { item, path, status });
    }
    let mut masks: [Option<TextureLevel>; 2] = [None, None];
    for (at, (bytes, name)) in [(&map.low_mask, "first"), (&map.high_mask, "second")]
        .into_iter()
        .enumerate()
        .take(if blend == Blend::Eight { 2 } else { 1 })
    {
        let status = match supercom_dds::decode(bytes) {
            _ if bytes.is_empty() => Status::Failed(format!("the map stores no {name} mask")),
            Err(error) => Status::Failed(format!("cannot decode: {error}")),
            Ok(decoded) => match levels_of(decoded).into_iter().next() {
                None => Status::Failed(format!("the {name} mask holds no level")),
                Some(level) => {
                    got.mask_bytes += bytes_of(std::slice::from_ref(&level));
                    masks[at] = Some(level);
                    Status::Ready
                }
            },
        };
        got.items.push(Checked {
            item: Item::Mask(at),
            path: String::new(),
            status,
        });
    }
    if decide(&map.terrain_program, &got.items) == Verdict::Textured(blend) {
        let [base, one, two, three, four, five, six, seven, eight, top] = layers;
        let [low, high] = masks;
        if let (Some(base), Some(top), Some(low)) = (base, top, low) {
            got.textures = Some(TerrainTextures {
                blend,
                filtering,
                base,
                layers: [one, two, three, four, five, six, seven, eight],
                top,
                low,
                high,
            });
        }
    }
    got
}

/// The fallback's notice for a set the renderer refused.
fn refused(error: &RenderError, map: &Map) -> Notice {
    let (item, path) = match error {
        RenderError::Texture {
            item: TextureItem::Layer(slot),
            ..
        } => (
            Item::Layer(*slot),
            map.strata
                .get(*slot)
                .map(|stratum| stratum.texture.clone())
                .unwrap_or_default(),
        ),
        RenderError::Texture {
            item: TextureItem::Mask(at),
            ..
        } => (Item::Mask(*at), String::new()),
        _ => (Item::Set, String::new()),
    };
    Notice {
        item,
        path,
        reason: error.to_string(),
        failed: 1,
    }
}

/// The verdict once the renderer has answered a set it was handed: the
/// textured terrain when it accepted, the fallback, with the item the error
/// names and a count of one, when it refused.
fn after_renderer(blend: Blend, answer: Result<(), RenderError>, map: &Map) -> Verdict {
    match answer {
        Ok(()) => Verdict::Textured(blend),
        Err(error) => Verdict::Fallback(refused(&error, map)),
    }
}

/// Decides from what was gathered and hands the renderer the set or
/// nothing: a decision for the fallback, and a set the renderer refuses,
/// leave the renderer holding no textures.
fn settle(map: &Map, got: Gathered, renderer: &mut Renderer) -> Resolution {
    let decided = decide(&map.terrain_program, &got.items);
    let verdict = match (decided, got.textures) {
        (Verdict::Textured(blend), Some(set)) => {
            after_renderer(blend, renderer.set_terrain_textures(&set), map)
        }
        (Verdict::Fallback(notice), _) => Verdict::Fallback(notice),
        (Verdict::Textured(_), None) => Verdict::Fallback(Notice {
            item: Item::Set,
            path: String::new(),
            reason: "the set could not be built".to_owned(),
            failed: 1,
        }),
    };
    if matches!(verdict, Verdict::Fallback(_)) {
        renderer.clear_terrain_textures();
    }
    Resolution {
        verdict,
        files_read: got.files_read,
        texture_bytes: got.texture_bytes,
        mask_bytes: got.mask_bytes,
    }
}

/// Reads, decodes, decides, and hands the renderer the set or nothing.
#[cfg(test)]
pub fn resolve(
    map: &Map,
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    renderer: &mut Renderer,
    filtering: Filtering,
) -> Resolution {
    let got = gather(map, read, filtering);
    settle(map, got, renderer)
}

/// The filtering of the binary's own lookups: linear, with the levels.
const RUN_FILTERING: Filtering = Filtering::Linear;

/// Reads and decodes what `map`'s program draws, as a run asks for it: the
/// filtered lookups.
fn run_gather(map: &Map, read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>) -> Gathered {
    gather(map, read, RUN_FILTERING)
}

/// The step of a run between the renderer and the terrain mesh, the same for
/// `view` and for `replay`: the grid fit check against the renderer's
/// device, then, only if the map fits, the textures read through `read`, and
/// the notice written to stderr, once, when the terrain is drawn by the height
/// ramp.
///
/// # Errors
/// [`RunError::Render`] when the grid does not fit the device, before any
/// file is read.
pub fn terrain(
    map: &Map,
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    renderer: &mut Renderer,
) -> Result<Verdict, RunError> {
    grid_fits(
        map.heightmap.width,
        map.heightmap.height,
        renderer.device().limits().max_buffer_size,
    )?;
    let got = run_gather(map, read);
    let resolution = settle(map, got, renderer);
    if let Verdict::Fallback(notice) = &resolution.verdict {
        eprintln!("supercom: {notice}");
    }
    Ok(resolution.verdict)
}

/// The step of `view`: [`terrain`] reading through the install's archives,
/// which are mounted at the first read, so a map that does not fit the device
/// mounts nothing. An install that does not mount makes every file missing.
///
/// # Errors
/// As [`terrain`].
pub fn load(
    map: &Map,
    install: &std::path::Path,
    renderer: &mut Renderer,
) -> Result<Verdict, RunError> {
    let mut vfs: Option<Result<supercom_scd::Vfs, String>> = None;
    let mut read = |path: &str| {
        let mounted = vfs.get_or_insert_with(|| {
            supercom_scd::Vfs::from_install(install).map_err(|error| error.to_string())
        });
        match mounted {
            Ok(vfs) => vfs.read(path).map_err(|error| error.to_string()),
            Err(error) => Err(format!("the archives do not mount: {error}")),
        }
    };
    terrain(map, &mut read, renderer)
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program => write!(f, "program"),
            Self::Layer(slot) => write!(f, "slot {slot}"),
            Self::Mask(0) => write!(f, "first mask"),
            Self::Mask(_) => write!(f, "second mask"),
            Self::Set => write!(f, "the set"),
        }
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "terrain textures not used, the height ramp is drawn: {}",
            self.item
        )?;
        if !self.path.is_empty() {
            write!(f, " ({})", self.path)?;
        }
        write!(f, ": {} ({} failed)", self.reason, self.failed)
    }
}

#[cfg(test)]
mod tests;
