use super::*;
use crate::error::RunError;
use supercom_scmap::{Heightmap, Stratum};

fn ready(item: Item) -> Checked {
    Checked {
        item,
        path: format!("/p/{item:?}.dds"),
        status: Status::Ready,
    }
}

fn failed(item: Item, reason: &str) -> Checked {
    Checked {
        status: Status::Failed(reason.to_owned()),
        ..ready(item)
    }
}

fn unnamed(item: Item) -> Checked {
    Checked {
        path: String::new(),
        status: Status::Unnamed,
        ..ready(item)
    }
}

/// Every item the first program draws, ready: slots 0 to 4 and 9, the first
/// mask.
fn four() -> Vec<Checked> {
    [0, 1, 2, 3, 4, 9]
        .into_iter()
        .map(|slot| ready(Item::Layer(slot)))
        .chain([ready(Item::Mask(0))])
        .collect()
}

/// Every item the second program draws, ready.
fn eight() -> Vec<Checked> {
    (0..10)
        .map(|slot| ready(Item::Layer(slot)))
        .chain([ready(Item::Mask(0)), ready(Item::Mask(1))])
        .collect()
}

fn replace(mut items: Vec<Checked>, with: &Checked) -> Vec<Checked> {
    for item in &mut items {
        if item.item == with.item {
            *item = with.clone();
        }
    }
    items
}

fn fallback(verdict: &Verdict) -> Option<&Notice> {
    match verdict {
        Verdict::Fallback(notice) => Some(notice),
        Verdict::Textured(_) => None,
    }
}

// REN-110
#[test]
fn supercom_ground_program_names_choose_the_blend() {
    assert_eq!(blend_for("TTerrain"), Some(Blend::Four));
    assert_eq!(blend_for("TTerrainGlow"), Some(Blend::Four));
    assert_eq!(blend_for("TTerrainXP"), Some(Blend::Eight));
    for other in [
        "",
        "terrain",
        "TTerrainxp",
        "TTerrain ",
        "TTerrainXP2",
        "Water",
    ] {
        assert_eq!(blend_for(other), None, "{other:?} is none of the three");
    }
}

// REN-114
#[test]
fn supercom_ground_path_normalization_cases() {
    for (stored, want) in [
        ("/Env/Layers/Grass.DDS", "env/layers/grass.dds"),
        ("\\Evil\\Layers\\A.dds", "/evil/layers/a.dds"),
        ("env/x.dds", "env/x.dds"),
        ("//a/B", "/a/b"),
        ("", ""),
    ] {
        assert_eq!(normalize(stored), want, "{stored:?}");
    }
}

// REN-111
#[test]
fn supercom_ground_decision_cases() {
    let textured = |program: &str, items: &[Checked]| decide(program, items);
    assert_eq!(
        textured("TTerrain", &four()),
        Verdict::Textured(Blend::Four)
    );
    assert_eq!(
        textured("TTerrainGlow", &four()),
        Verdict::Textured(Blend::Four)
    );
    assert_eq!(
        textured("TTerrainXP", &eight()),
        Verdict::Textured(Blend::Eight)
    );

    // The program is none of the three: the first case, one item failed.
    let verdict = textured("Other", &four());
    let notice = fallback(&verdict);
    assert_eq!(notice.map(|n| (n.item, n.failed)), Some((Item::Program, 1)));
    assert_eq!(notice.map(|n| n.path.as_str()), Some("Other"));

    // Each item of the first program, failed alone, is the whole fallback.
    for slot in [0, 1, 2, 3, 4, 9] {
        let verdict = textured(
            "TTerrain",
            &replace(four(), &failed(Item::Layer(slot), "gone")),
        );
        let notice = fallback(&verdict);
        assert_eq!(
            notice.map(|n| (n.item, n.failed, n.reason.as_str())),
            Some((Item::Layer(slot), 1, "gone")),
            "slot {slot}"
        );
    }
    let verdict = textured(
        "TTerrain",
        &replace(four(), &failed(Item::Mask(0), "refused")),
    );
    assert_eq!(fallback(&verdict).map(|n| n.item), Some(Item::Mask(0)));
    for slot in [5, 6, 7, 8] {
        let verdict = textured(
            "TTerrainXP",
            &replace(eight(), &failed(Item::Layer(slot), "gone")),
        );
        assert_eq!(fallback(&verdict).map(|n| n.item), Some(Item::Layer(slot)));
    }
    let verdict = textured(
        "TTerrainXP",
        &replace(eight(), &failed(Item::Mask(1), "refused")),
    );
    assert_eq!(fallback(&verdict).map(|n| n.item), Some(Item::Mask(1)));

    // Reading 1: an unnamed slot 0 or 9 is the fallback; an unnamed slot
    // from 1 to 8 its program blends is skipped.
    for slot in [0, 9] {
        let verdict = textured("TTerrain", &replace(four(), &unnamed(Item::Layer(slot))));
        assert_eq!(fallback(&verdict).map(|n| n.item), Some(Item::Layer(slot)));
    }
    for slot in [1, 3] {
        let items = replace(four(), &unnamed(Item::Layer(slot)));
        assert_eq!(textured("TTerrain", &items), Verdict::Textured(Blend::Four));
    }
    let items = replace(eight(), &unnamed(Item::Layer(6)));
    assert_eq!(
        textured("TTerrainXP", &items),
        Verdict::Textured(Blend::Eight)
    );

    // Reading 4: what the program does not read is neither a reason nor
    // looked at.
    let mut items = four();
    items.push(failed(Item::Mask(1), "never decoded"));
    items.push(failed(Item::Layer(6), "never read"));
    assert_eq!(textured("TTerrain", &items), Verdict::Textured(Blend::Four));
    assert_eq!(
        textured("TTerrainGlow", &items),
        Verdict::Textured(Blend::Four)
    );

    // The first failing item in the order program, slot 0 to 9, first mask,
    // second mask is named, and every failure is counted.
    let mut items = replace(eight(), &failed(Item::Mask(1), "m1"));
    items = replace(items, &failed(Item::Mask(0), "m0"));
    items = replace(items, &failed(Item::Layer(9), "s9"));
    items = replace(items, &failed(Item::Layer(4), "s4"));
    items = replace(items, &unnamed(Item::Layer(0)));
    let verdict = textured("TTerrainXP", &items);
    assert_eq!(
        fallback(&verdict).map(|n| (n.item, n.failed)),
        Some((Item::Layer(0), 5))
    );
    let verdict = textured("TTerrainXP", &replace(items, &ready(Item::Layer(0))));
    assert_eq!(
        fallback(&verdict).map(|n| (n.item, n.failed, n.reason.as_str())),
        Some((Item::Layer(4), 4, "s4"))
    );
}

// REN-113
#[test]
fn supercom_ground_notice_names_the_item_the_file_the_reason_and_the_count() {
    let notice = Notice {
        item: Item::Layer(3),
        path: "/maps/x/y.dds".to_owned(),
        reason: "cannot read: nope".to_owned(),
        failed: 2,
    };
    assert_eq!(
        notice.to_string(),
        "terrain textures not used, the height ramp is drawn: slot 3 (/maps/x/y.dds): cannot read: nope (2 failed)"
    );
    let mask = Notice {
        item: Item::Mask(1),
        path: String::new(),
        reason: "the map stores no second mask".to_owned(),
        failed: 1,
    };
    assert_eq!(
        mask.to_string(),
        "terrain textures not used, the height ramp is drawn: second mask: the map stores no second mask (1 failed)"
    );
}

// ---- gathering from a map with literal files ---------------------------

const HEADER: usize = 128;

/// A 4 by 4 DXT1 file of one level: 128 header bytes and 8 zero data bytes,
/// which decodes to 4 * 4 * 4 = 64 bytes.
fn dds() -> Vec<u8> {
    let mut out = b"DDS ".to_vec();
    for word in [124_u32, 0x1007, 4, 4, 0, 0, 0] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.resize(76, 0);
    out.extend_from_slice(&32_u32.to_le_bytes());
    out.extend_from_slice(&4_u32.to_le_bytes());
    out.extend_from_slice(b"DXT1");
    out.resize(HEADER + 8, 0);
    out
}

fn map_named(program: &str, named: &[usize], scale: f32) -> Map {
    Map {
        width: 64.0,
        height: 64.0,
        version: 60,
        preview: Vec::new(),
        heightmap: Heightmap {
            width: 1,
            height: 1,
            scale: 1.0,
            samples: vec![0; 4],
        },
        strata: (0..10)
            .map(|slot| Stratum {
                texture: if named.contains(&slot) {
                    format!("\\Env\\Layer{slot}.DDS")
                } else {
                    String::new()
                },
                scale,
            })
            .collect(),
        water: None,
        terrain_program: program.to_owned(),
        low_mask: dds(),
        high_mask: dds(),
    }
}

fn all() -> Vec<usize> {
    (0..10).collect()
}

fn reader(log: &mut Vec<String>) -> impl FnMut(&str) -> Result<Vec<u8>, String> + '_ {
    move |path| {
        log.push(path.to_owned());
        Ok(dds())
    }
}

// REN-114
#[test]
fn supercom_ground_gather_reads_only_what_the_program_draws() {
    let mut log = Vec::new();
    let got = gather(
        &map_named("TTerrain", &all(), 4.0),
        &mut reader(&mut log),
        Filtering::Linear,
    );
    assert_eq!(
        log,
        [
            "/env/layer0.dds",
            "/env/layer1.dds",
            "/env/layer2.dds",
            "/env/layer3.dds",
            "/env/layer4.dds",
            "/env/layer9.dds"
        ],
        "slots 0 to 4 and 9, normalized, in order"
    );
    assert_eq!(
        (got.files_read, got.texture_bytes, got.mask_bytes),
        (6, 384, 64)
    );
    assert!(got.textures.is_some(), "every item is ready");
    let mut log = Vec::new();
    let got = gather(
        &map_named("TTerrainXP", &all(), 4.0),
        &mut reader(&mut log),
        Filtering::Linear,
    );
    assert_eq!(log.len(), 10);
    assert_eq!(
        (got.files_read, got.texture_bytes, got.mask_bytes),
        (10, 640, 128)
    );
    let mut log = Vec::new();
    let got = gather(
        &map_named("Unknown", &all(), 4.0),
        &mut reader(&mut log),
        Filtering::Linear,
    );
    assert_eq!(
        (log.len(), got.files_read, got.mask_bytes),
        (0, 0, 0),
        "an unknown program reads nothing"
    );
    assert!(got.textures.is_none());
    assert_eq!(
        fallback(&decide("Unknown", &got.items)).map(|n| n.item),
        Some(Item::Program)
    );
}

// REN-112
#[test]
fn supercom_ground_gather_reports_each_reading_of_the_fallback() {
    let verdict = |map: &Map, read: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>| {
        let got = gather(map, read, Filtering::Linear);
        (decide(&map.terrain_program, &got.items), got)
    };
    // Reading 3: nothing mounts, so every named file is missing.
    let mut none = |_: &str| Err("the archives do not mount".to_owned());
    let (v, got) = verdict(&map_named("TTerrain", &all(), 4.0), &mut none);
    let notice = fallback(&v);
    assert_eq!(
        notice.map(|n| (n.item, n.failed)),
        Some((Item::Layer(0), 6))
    );
    assert_eq!(
        notice.map(|n| (n.path.as_str(), n.reason.contains("do not mount"))),
        Some(("\\Env\\Layer0.DDS", true))
    );
    assert!(got.textures.is_none());
    // Reading 2: a scale that is not finite and positive.
    for scale in [f32::NAN, 0.0, -2.0, f32::INFINITY] {
        let mut log = Vec::new();
        let (v, _) = verdict(&map_named("TTerrain", &all(), scale), &mut reader(&mut log));
        let notice = fallback(&v);
        assert_eq!(
            notice.map(|n| n.item),
            Some(Item::Layer(0)),
            "scale {scale}"
        );
        assert!(
            notice.is_some_and(|n| n.reason.contains("scale")),
            "the reason names the scale: {notice:?}"
        );
    }
    // A file the decoder refuses.
    let mut bad = |_: &str| Ok(b"not a texture".to_vec());
    let (v, _) = verdict(&map_named("TTerrain", &all(), 4.0), &mut bad);
    assert_eq!(fallback(&v).map(|n| n.item), Some(Item::Layer(0)));
    // Reading 1: slot 0 or 9 unnamed.
    for slot in [0, 9] {
        let named: Vec<usize> = all().into_iter().filter(|s| *s != slot).collect();
        let mut log = Vec::new();
        let (v, _) = verdict(&map_named("TTerrain", &named, 4.0), &mut reader(&mut log));
        assert_eq!(fallback(&v).map(|n| n.item), Some(Item::Layer(slot)));
    }
    // An unnamed slot from 1 to 8 is skipped: the set holds nothing there.
    let named: Vec<usize> = all().into_iter().filter(|s| *s != 3).collect();
    let mut log = Vec::new();
    let (v, got) = verdict(&map_named("TTerrain", &named, 4.0), &mut reader(&mut log));
    assert_eq!(v, Verdict::Textured(Blend::Four));
    assert_eq!(got.textures.map(|t| t.layers[2].is_none()), Some(true));
    // A mask the map does not hold, and a mask the decoder refuses.
    let mut map = map_named("TTerrain", &all(), 4.0);
    map.low_mask.clear();
    let mut log = Vec::new();
    let (v, _) = verdict(&map, &mut reader(&mut log));
    assert_eq!(fallback(&v).map(|n| n.item), Some(Item::Mask(0)));
    let mut map = map_named("TTerrainXP", &all(), 4.0);
    map.high_mask = b"garbage".to_vec();
    let (v, _) = verdict(&map, &mut reader(&mut log));
    assert_eq!(fallback(&v).map(|n| n.item), Some(Item::Mask(1)));
    // Reading 4: the first program neither decodes nor needs the second mask.
    let mut map = map_named("TTerrain", &all(), 4.0);
    map.high_mask = b"garbage".to_vec();
    let (v, got) = verdict(&map, &mut reader(&mut log));
    assert_eq!(v, Verdict::Textured(Blend::Four));
    assert_eq!(got.mask_bytes, 64, "only the first mask is decoded");
}

// REN-116
#[test]
fn supercom_ground_hands_the_renderer_the_set_or_nothing() {
    use supercom_render::{GpuGate, gpu_gate};
    let made = Renderer::new(supercom_render::wgpu::TextureFormat::Rgba8UnormSrgb);
    let found = !matches!(made, Err(supercom_render::Error::NoAdapter { .. }));
    let require = std::env::var("SUPERCOM_REQUIRE_GPU").is_ok_and(|value| value == "1");
    match gpu_gate(found, require) {
        GpuGate::Skip(reason) => return eprintln!("SKIP: {reason}"),
        gate => assert_eq!(gate, GpuGate::Run, "an adapter is required"),
    }
    let mut renderer = made.expect("the renderer is built on the adapter found");
    let mut log = Vec::new();
    let got = resolve(
        &map_named("TTerrainXP", &all(), 4.0),
        &mut reader(&mut log),
        &mut renderer,
        Filtering::Linear,
    );
    assert_eq!(got.verdict, Verdict::Textured(Blend::Eight));
    assert!(renderer.terrain_textured(), "the set was handed over");
    let got = resolve(
        &map_named("Other", &all(), 4.0),
        &mut reader(&mut log),
        &mut renderer,
        Filtering::Linear,
    );
    assert_eq!(fallback(&got.verdict).map(|n| n.item), Some(Item::Program));
    assert!(
        !renderer.terrain_textured(),
        "a fallback leaves nothing handed over"
    );
}

// ---- the installed game -------------------------------------------------

fn mount(fa: &std::path::Path) -> supercom_scd::Vfs {
    supercom_scd::Vfs::from_install(fa).expect("the install's archives mount")
}

fn installed_map(fa: &std::path::Path, folder: &str) -> Map {
    crate::plan::read_map(fa, folder).expect("the installed map reads and parses")
}

// REN-092
#[test]
fn conformance_ground_terrain_member_is_pinned() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let archive = supercom_scd::Archive::open(fa.join("gamedata").join("effects.scd"))
        .expect("effects.scd opens");
    let name = archive
        .entries()
        .map(|entry| entry.name.clone())
        .find(|name| name.eq_ignore_ascii_case("effects/terrain.fx"))
        .expect("effects.scd holds effects/terrain.fx");
    let bytes = archive.read(&name).expect("the member reads");
    let newlines = bytes
        .iter()
        .fold(0_usize, |n, b| n + usize::from(*b == b'\n'));
    assert_eq!((bytes.len(), newlines), (47_280, 1_567));
}

// REN-115
#[test]
fn conformance_ground_installed_reads_only_what_the_program_draws() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    for folder in ["SCMP_012", "SCMP_016"] {
        let map = installed_map(&fa, folder);
        assert_eq!(map.terrain_program, "TTerrain", "{folder}");
        for run in ["first", "second"] {
            let started = std::time::Instant::now();
            let vfs = mount(&fa);
            let mounted = started.elapsed();
            let mut files = Vec::new();
            let mut read = |path: &str| {
                files.push(path.to_owned());
                vfs.read(path).map_err(|error| error.to_string())
            };
            let got = gather(&map, &mut read, Filtering::Linear);
            println!(
                "GROUND_TIMING {folder} {run}: mount {mounted:?}, read and decode {:?}",
                started.elapsed().saturating_sub(mounted)
            );
            assert_eq!(files.len(), 6, "{folder}: six texture files are read");
            assert_eq!(
                (got.files_read, got.texture_bytes, got.mask_bytes),
                (6, 7_340_024, 65_536),
                "{folder}"
            );
            assert_eq!(
                decide(&map.terrain_program, &got.items),
                Verdict::Textured(Blend::Four),
                "{folder}: no notice"
            );
            assert!(got.textures.is_some(), "{folder}: the set is built");
        }
    }
}

// REN-096
#[test]
fn conformance_ground_installed_unnamed_slot_masks_weigh_nothing() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let mut folders: Vec<String> = std::fs::read_dir(fa.join("maps"))
        .expect("the maps directory lists")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    folders.sort();
    let (mut largest, mut counted) = (0_u8, 0_u32);
    for folder in &folders {
        let Ok(map) = crate::plan::read_map(&fa, folder) else {
            continue;
        };
        if map.terrain_program != "TTerrainXP" {
            continue;
        }
        let levels = supercom_dds::decode(&map.high_mask).expect("the second mask decodes");
        for texel in levels[0].rgba.chunks_exact(4) {
            for (k, value) in texel.iter().enumerate() {
                if map.strata[5 + k].texture.is_empty() {
                    largest = largest.max(*value);
                }
            }
        }
        counted += 1;
    }
    assert_eq!(counted, 20, "TTerrainXP is named by 20 maps");
    assert_eq!(largest, 34, "the largest value under an unnamed slot");
}

fn installed_frames(fa: &std::path::Path, folder: &str) -> (Vec<[u8; 3]>, Vec<[u8; 3]>) {
    use supercom_render::{Camera, GpuGate, TerrainMesh, WaterPlane, gpu_gate};
    let made = Renderer::new(supercom_render::wgpu::TextureFormat::Rgba8UnormSrgb);
    let found = !matches!(made, Err(supercom_render::Error::NoAdapter { .. }));
    let require = std::env::var("SUPERCOM_REQUIRE_GPU").is_ok_and(|value| value == "1");
    match gpu_gate(found, require) {
        GpuGate::Skip(reason) => {
            eprintln!("SKIP: {reason}");
            return (Vec::new(), Vec::new());
        }
        gate => assert_eq!(gate, GpuGate::Run, "an adapter is required"),
    }
    let mut renderer = made.expect("the renderer is built on the adapter found");
    let map = installed_map(fa, folder);
    let mesh = TerrainMesh::build(&map).expect("the map builds");
    let water = WaterPlane::from_map(&map);
    renderer
        .upload(&mesh, water.as_ref())
        .expect("the terrain uploads");
    let camera = Camera::for_mesh(&mesh, 1.0).expect("the strategic camera");
    let ramp = crate::testing::draw_offscreen(&mut renderer, &camera, 512);
    let vfs = mount(fa);
    let mut read = |path: &str| vfs.read(path).map_err(|error| error.to_string());
    let got = resolve(&map, &mut read, &mut renderer, Filtering::Linear);
    assert_eq!(
        got.verdict,
        Verdict::Textured(Blend::Four),
        "{folder}: no notice"
    );
    let textured = crate::testing::draw_offscreen(&mut renderer, &camera, 512);
    (ramp, textured)
}

/// The pixels of the middle 80 percent of a 512 square that differ from the
/// other frame's by more than 2 in 255 in a channel, over the pixels of the
/// middle.
fn middle_difference(a: &[[u8; 3]], b: &[[u8; 3]]) -> f64 {
    let (mut differ, mut total) = (0_u32, 0_u32);
    for y in 51..461 {
        for x in 51..461 {
            let (p, q) = (a[y * 512 + x], b[y * 512 + x]);
            total += 1;
            differ += u32::from(p.iter().zip(q).any(|(p, q)| p.abs_diff(q) > 2));
        }
    }
    f64::from(differ) / f64::from(total)
}

// REN-117
#[test]
fn conformance_gpu_ground_installed_scmp_012_draws_the_textured_terrain() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (ramp, textured) = installed_frames(&fa, "SCMP_012");
    if ramp.is_empty() {
        return;
    }
    let share = middle_difference(&ramp, &textured);
    println!("GROUND_FRAME SCMP_012 middle share differing from the ramp: {share}");
    assert!(
        share > 0.5,
        "the middle is not the fallback's frame: {share}"
    );
}

// REN-117
#[test]
fn conformance_gpu_ground_installed_scmp_016_draws_the_textured_terrain_and_its_water() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let (ramp, textured) = installed_frames(&fa, "SCMP_016");
    if ramp.is_empty() {
        return;
    }
    let share = middle_difference(&ramp, &textured);
    println!("GROUND_FRAME SCMP_016 middle share differing from the ramp: {share}");
    assert!(
        share > 0.5,
        "the middle is not the fallback's frame: {share}"
    );
    // The water's colour, linear (0.04, 0.22, 0.45), encoded for sRGB.
    let encode = |c: f64| {
        255.0
            * if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
    };
    let water = [encode(0.04), encode(0.22), encode(0.45)];
    let clear = [encode(0.01), encode(0.01), encode(0.02)];
    let is = |p: [u8; 3], want: [f64; 3]| {
        p.iter()
            .zip(want)
            .all(|(p, w)| (f64::from(*p) - w).abs() <= 2.0)
    };
    let covered = textured.iter().filter(|p| !is(**p, clear)).count();
    let wet = textured.iter().filter(|p| is(**p, water)).count();
    let count = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    let fraction = count(wet) / count(covered);
    println!("GROUND_FRAME SCMP_016 water share: {fraction}");
    assert!(
        (fraction - 0.095).abs() <= 0.03,
        "the water covers {fraction} of the covered pixels, REN-031 says 0.095 within 0.03"
    );
}

/// A renderer on the adapter found, or `None` after the skip line.
fn adapter_renderer() -> Option<Renderer> {
    use supercom_render::{GpuGate, gpu_gate};
    let made = Renderer::new(supercom_render::wgpu::TextureFormat::Rgba8UnormSrgb);
    let found = !matches!(made, Err(supercom_render::Error::NoAdapter { .. }));
    let require = std::env::var("SUPERCOM_REQUIRE_GPU").is_ok_and(|value| value == "1");
    match gpu_gate(found, require) {
        GpuGate::Skip(reason) => {
            eprintln!("SKIP: {reason}");
            None
        }
        gate => {
            assert_eq!(gate, GpuGate::Run, "an adapter is required");
            Some(made.expect("the renderer is built on the adapter found"))
        }
    }
}

/// Pins REN-035's order: the grid fit check against the renderer's device
/// comes before the map's textures are read. A map of 4096 cells a side is
/// refused (REN-024), and the run ends with that error before any file is
/// asked for.
#[test]
fn supercom_ground_fit_check_comes_before_any_file_is_read() {
    let Some(mut renderer) = adapter_renderer() else {
        return;
    };
    let mut map = map_named("TTerrain", &all(), 4.0);
    map.heightmap.width = 4096;
    map.heightmap.height = 4096;
    let mut asked = Vec::new();
    let mut read = |path: &str| {
        asked.push(path.to_owned());
        Ok(dds())
    };
    let got = terrain(&map, &mut read, &mut renderer);
    assert!(
        matches!(
            got,
            Err(RunError::Render(
                supercom_render::Error::GridTooLarge { .. }
            ))
        ),
        "the grid is refused: {got:?}"
    );
    assert!(asked.is_empty(), "no file is read: {asked:?}");
    assert!(!renderer.terrain_textured(), "nothing was handed over");
    // A map that fits reads its six files after the check.
    let fits = map_named("TTerrain", &all(), 4.0);
    let mut asked = Vec::new();
    let mut read = |path: &str| {
        asked.push(path.to_owned());
        Ok(dds())
    };
    let got = terrain(&fits, &mut read, &mut renderer);
    assert_eq!(got.ok(), Some(Verdict::Textured(Blend::Four)));
    assert_eq!(asked.len(), 6);
}
