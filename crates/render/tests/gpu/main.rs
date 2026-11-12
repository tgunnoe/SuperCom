//! The GPU side against the crate's pure functions. The `conformance_gpu_`
//! tests draw offscreen and read the frame back; they depend on an adapter,
//! not on the install (the two that also read the install call
//! `require_game` first). With no adapter they print `SKIP: <reason>` and
//! pass; with `SUPERCOM_REQUIRE_GPU=1` they fail instead. When
//! `SUPERCOM_RENDER_DUMP` names an absolute directory outside the
//! repository, each frame read back is also written there as a binary PPM.

use std::ffi::OsString;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::{env, fs, sync::mpsc};
use supercom_render::{
    AMBIENT, CLEAR_COLOR, Camera, Error, GpuGate, LIGHT_DIRECTION, RAMP, Renderer, TerrainMesh,
    WATER_COLOR, WaterPlane, ZoomSpeed, gpu_gate, grid_fits, normalize_height, shade,
    shader_source,
};
use supercom_scmap::{Heightmap, Map, Water};
use wgpu::{
    Backends, BufferDescriptor, BufferUsages, Extent3d, MapMode, Origin3d, PollType,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};

type Rgb = [u8; 3];

/// Per-channel tolerance in 255ths: a cell of rounding to 8 bits plus the
/// GPU's arithmetic against the CPU's.
const TOLERANCE: f64 = 2.0;

/// The default maximum buffer size wgpu requests: 256 MiB.
const DEFAULT_MAX_BUFFER: u64 = 256 << 20;

#[expect(
    clippy::panic,
    reason = "test support; clippy.toml exempts only #[test] functions"
)]
fn fail(message: &str) -> ! {
    panic!("{message}")
}

#[expect(
    clippy::expect_used,
    reason = "test support; clippy.toml exempts only #[test] functions"
)]
#[track_caller]
fn must<T, E: Debug>(result: Result<T, E>, what: &str) -> T {
    result.expect(what)
}

fn to_usize(n: u32) -> usize {
    must(usize::try_from(n), "a u32 fits usize")
}

/// A pixel coordinate from a non-negative real, clamped to `0..limit`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to the pixel range before the cast"
)]
fn pixel_index(v: f64, limit: u32) -> u32 {
    (v.floor().max(0.0) as u32).min(limit - 1)
}

// ---- the adapter and the skip rule ------------------------------------

fn require_gpu() -> bool {
    env::var("SUPERCOM_REQUIRE_GPU").is_ok_and(|value| value == "1")
}

/// A renderer for `format`, or `None` after printing `SKIP: <reason>` when
/// no adapter exists and none is required. Prints the adapter when one runs.
fn gpu(format: TextureFormat) -> Option<Renderer> {
    let made = Renderer::new(format);
    let found = !matches!(made, Err(Error::NoAdapter { .. }));
    match gpu_gate(found, require_gpu()) {
        GpuGate::Skip(reason) => {
            eprintln!("SKIP: {reason}");
            None
        }
        GpuGate::Fail(reason) => fail(&reason),
        GpuGate::Run => {
            let renderer = must(made, "the renderer is built on the adapter found");
            println!(
                "adapter: {} ({:?}), format {format:?}",
                renderer.adapter_name(),
                renderer.backend()
            );
            Some(renderer)
        }
    }
}

// ---- colours -----------------------------------------------------------

/// The 0-to-255 value a linear channel reads back as in a format with or
/// without the sRGB transfer function.
fn encode(linear: f32, srgb: bool) -> f64 {
    let l = f64::from(linear).clamp(0.0, 1.0);
    let v = if srgb {
        if l <= 0.003_130_8 {
            l * 12.92
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        }
    } else {
        l
    };
    v * 255.0
}

fn expected(color: [f32; 3], format: TextureFormat) -> [f64; 3] {
    color.map(|c| encode(c, format.is_srgb()))
}

fn near(pixel: Rgb, want: [f64; 3]) -> bool {
    pixel
        .iter()
        .zip(want)
        .all(|(&got, want)| (f64::from(got) - want).abs() <= TOLERANCE)
}

fn luminance(pixel: Rgb) -> i32 {
    pixel.iter().map(|&c| i32::from(c)).sum()
}

// ---- frames ------------------------------------------------------------

struct Frame {
    format: TextureFormat,
    width: u32,
    height: u32,
    pixels: Vec<Rgb>,
}

impl Frame {
    fn at(&self, x: u32, y: u32) -> Rgb {
        self.pixels[to_usize(y * self.width + x)]
    }

    fn is_clear(&self, x: u32, y: u32) -> bool {
        near(self.at(x, y), expected(CLEAR_COLOR, self.format))
    }

    fn is_water(&self, x: u32, y: u32) -> bool {
        near(self.at(x, y), expected(WATER_COLOR, self.format))
    }

    fn is_terrain(&self, x: u32, y: u32) -> bool {
        !self.is_clear(x, y) && !self.is_water(x, y)
    }

    /// The first row from the top, and the last from the bottom, in column
    /// `x` that is not the clear colour.
    fn column_span(&self, x: u32) -> Option<(u32, u32)> {
        let first = (0..self.height).find(|&y| !self.is_clear(x, y))?;
        let last = (0..self.height).rev().find(|&y| !self.is_clear(x, y))?;
        Some((first, last))
    }

    /// The first column from the left, and the last from the right, in row
    /// `y` that is not the clear colour.
    fn row_span(&self, y: u32) -> Option<(u32, u32)> {
        let first = (0..self.width).find(|&x| !self.is_clear(x, y))?;
        let last = (0..self.width).rev().find(|&x| !self.is_clear(x, y))?;
        Some((first, last))
    }

    fn count(&self, test: impl Fn(&Self, u32, u32) -> bool) -> usize {
        let mut n = 0;
        for y in 0..self.height {
            for x in 0..self.width {
                n += usize::from(test(self, x, y));
            }
        }
        n
    }
}

/// Draws one frame into a fresh offscreen texture of the renderer's format
/// and reads it back with row 0 at the top.
fn draw_frame(
    renderer: &mut Renderer,
    camera: &Camera,
    (width, height): (u32, u32),
    label: &str,
) -> Frame {
    let format = renderer.format();
    let device = renderer.device().clone();
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("offscreen"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    must(
        renderer.draw(camera, &view, (width, height)),
        "the draw call accepts a well-formed target",
    );

    let row_bytes = (width * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row_bytes) * u64::from(height),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
        },
        size,
    );
    renderer.queue().submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    buffer.map_async(MapMode::Read, .., move |result| {
        // The receiver outlives the wait below; a failed send only means
        // the test already failed.
        drop(sender.send(result));
    });
    must(
        device.poll(PollType::wait_indefinitely()),
        "the device finishes the copy",
    );
    must(
        must(receiver.recv(), "the map callback runs"),
        "the readback buffer maps",
    );
    let swap = matches!(
        format,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
    );
    let mapped = must(buffer.get_mapped_range(..), "the buffer is mapped");
    let mut pixels = Vec::new();
    for y in 0..height {
        let start = to_usize(y * row_bytes);
        for x in 0..width {
            let at = start + to_usize(x) * 4;
            let (a, b, c) = (mapped[at], mapped[at + 1], mapped[at + 2]);
            pixels.push(if swap { [c, b, a] } else { [a, b, c] });
        }
    }
    drop(mapped);
    let frame = Frame {
        format,
        width,
        height,
        pixels,
    };
    dump(&frame, label);
    frame
}

/// The directory `SUPERCOM_RENDER_DUMP` names, or `None` when unset; an
/// error when it is not absolute or lies inside the repository.
fn dump_directory(var: Option<OsString>, root: &Path) -> Result<Option<PathBuf>, String> {
    let Some(var) = var.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let dir = PathBuf::from(var);
    if !dir.is_absolute() {
        return Err(format!(
            "SUPERCOM_RENDER_DUMP {} is not an absolute directory",
            dir.display()
        ));
    }
    if dir.starts_with(root) {
        return Err(format!(
            "SUPERCOM_RENDER_DUMP {} lies inside the repository {}; a dumped frame must never be tracked",
            dir.display(),
            root.display()
        ));
    }
    Ok(Some(dir))
}

/// Writes `frame` as a binary PPM named for `label` when the dump variable
/// is set; writes nothing when it is not.
fn dump(frame: &Frame, label: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(
            || fail("the crate sits two levels below the root"),
            Path::to_path_buf,
        );
    let dir = match dump_directory(env::var_os("SUPERCOM_RENDER_DUMP"), &root) {
        Ok(Some(dir)) => dir,
        Ok(None) => return,
        Err(why) => fail(&why),
    };
    must(fs::create_dir_all(&dir), "the dump directory is created");
    let mut bytes = format!("P{}\n{} {}\n255\n", 6, frame.width, frame.height).into_bytes();
    bytes.extend(frame.pixels.iter().flatten());
    let name = label.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_");
    must(
        fs::write(dir.join(format!("{name}.ppm")), bytes),
        "the frame is written",
    );
}

// ---- synthetic maps ----------------------------------------------------

/// A square map of `cells` by `cells` over `extent` world units.
fn map_of(cells: u32, extent: f32, scale: f32, sample: impl Fn(u32, u32) -> i16) -> Map {
    let mut samples = Vec::new();
    for j in 0..=cells {
        for i in 0..=cells {
            samples.push(sample(i, j));
        }
    }
    Map {
        width: extent,
        height: extent,
        version: 60,
        preview: Vec::new(),
        heightmap: Heightmap {
            width: cells,
            height: cells,
            scale,
            samples,
        },
        strata: Vec::new(),
        water: None,
    }
}

fn with_water(mut map: Map, elevation: f32) -> Map {
    map.water = Some(Water {
        elevation,
        deep: elevation - 2.0,
        abyss: elevation - 4.0,
    });
    map
}

fn mesh_of(map: &Map) -> TerrainMesh {
    must(TerrainMesh::build(map), "the synthetic map builds")
}

fn scene(renderer: &mut Renderer, map: &Map, with_plane: bool) -> TerrainMesh {
    let mesh = mesh_of(map);
    let plane = if with_plane {
        WaterPlane::from_map(map)
    } else {
        None
    };
    must(renderer.upload(&mesh, plane.as_ref()), "the mesh uploads");
    mesh
}

// ---- projecting by the camera's own matrix, in f64 ---------------------

type Matrix = [[f32; 4]; 4];

/// Normalized device x and y of a world point: the matrix times the column
/// vector, `m[c][r]` being row `r` of column `c`.
fn project(matrix: &Matrix, point: [f64; 3]) -> [f64; 2] {
    let row = |r: usize| {
        f64::from(matrix[0][r]) * point[0]
            + f64::from(matrix[1][r]) * point[1]
            + f64::from(matrix[2][r]) * point[2]
            + f64::from(matrix[3][r])
    };
    [row(0) / row(3), row(1) / row(3)]
}

/// The normalized device position of the centre of pixel `(x, y)`.
fn centre_ndc(x: u32, y: u32, (width, height): (u32, u32)) -> [f64; 2] {
    [
        (f64::from(x) + 0.5) / f64::from(width) * 2.0 - 1.0,
        1.0 - (f64::from(y) + 0.5) / f64::from(height) * 2.0,
    ]
}

/// The pixel a world point falls in.
fn pixel_of(matrix: &Matrix, point: [f64; 3], (width, height): (u32, u32)) -> (u32, u32) {
    let [nx, ny] = project(matrix, point);
    (
        pixel_index(f64::midpoint(nx, 1.0) * f64::from(width), width),
        pixel_index(f64::midpoint(1.0, -ny) * f64::from(height), height),
    )
}

/// The `(x, z)` of the point of the surface `y = surface(x, z)` that the
/// centre of pixel `(px, py)` sees, found by Newton's method from `guess`
/// with a finite-difference Jacobian.
fn surface_under(
    matrix: &Matrix,
    target: [f64; 2],
    surface: &dyn Fn(f64, f64) -> f64,
    guess: (f64, f64),
) -> (f64, f64) {
    let miss = |at: (f64, f64)| {
        let ndc = project(matrix, [at.0, surface(at.0, at.1), at.1]);
        [ndc[0] - target[0], ndc[1] - target[1]]
    };
    let mut at = guess;
    let step = 1e-3;
    for _ in 0..8 {
        let f0 = miss(at);
        let (fx, fz) = (miss((at.0 + step, at.1)), miss((at.0, at.1 + step)));
        let (j00, j01) = ((fx[0] - f0[0]) / step, (fz[0] - f0[0]) / step);
        let (j10, j11) = ((fx[1] - f0[1]) / step, (fz[1] - f0[1]) / step);
        let det = j00 * j11 - j01 * j10;
        at.0 -= (f0[0] * j11 - j01 * f0[1]) / det;
        at.1 -= (j00 * f0[1] - j10 * f0[0]) / det;
    }
    at
}

/// The pixel a point on the surface at fractions `(u, v)` of the extent
/// falls in, and the colour the pure shading function gives the surface
/// point that pixel's centre sees: the normalized height from the mesh's
/// range, the normal from the mesh (exact on a plane).
fn expected_on_plane(
    mesh: &TerrainMesh,
    matrix: &Matrix,
    size: (u32, u32),
    surface: &dyn Fn(f64, f64) -> f64,
    (across, down): (f64, f64),
) -> ((u32, u32), [f32; 3], f32) {
    let nominal = (
        across * f64::from(mesh.width),
        down * f64::from(mesh.height),
    );
    let at = pixel_of(
        matrix,
        [nominal.0, surface(nominal.0, nominal.1), nominal.1],
        size,
    );
    let (x, z) = surface_under(matrix, centre_ndc(at.0, at.1, size), surface, nominal);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the shading function takes f32"
    )]
    let elevation = surface(x, z) as f32;
    let height = normalize_height(elevation, mesh.lowest, mesh.highest);
    let normal = mesh.normals[mesh.normals.len() / 2];
    (at, shade(height, normal), height)
}

// ---- REN-019 -----------------------------------------------------------

/// A plane rising east: 0.5 world units per unit of x.
fn east_ramp() -> Map {
    map_of(8, 64.0, 1.0, |i, _| 4 * i16::try_from(i).unwrap_or(0))
}

#[test]
fn conformance_gpu_one_draw_function_fills_views_of_any_size() {
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let mesh = scene(&mut renderer, &east_ramp(), false);
    for size in [(96, 72), (200, 120)] {
        let aspect = f32::from(u16::try_from(size.0).unwrap_or(1))
            / f32::from(u16::try_from(size.1).unwrap_or(1));
        let camera = must(Camera::for_mesh(&mesh, aspect), "the camera builds");
        let frame = draw_frame(
            &mut renderer,
            &camera,
            size,
            &format!("one-draw-{}x{}", size.0, size.1),
        );
        assert!(
            frame.is_terrain(size.0 / 2, size.1 / 2),
            "the centre of the {size:?} view is terrain, not {:?}",
            frame.at(size.0 / 2, size.1 / 2)
        );
        let terrain = frame.count(Frame::is_terrain);
        let all = to_usize(size.0 * size.1);
        assert!(
            terrain * 4 > all,
            "terrain covers {terrain} of {all} pixels of the {size:?} view"
        );
    }
}

// ---- REN-020 -----------------------------------------------------------

#[test]
fn conformance_gpu_empty_margins_hold_the_clear_colour() {
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let mesh = scene(&mut renderer, &east_ramp(), false);
    let size = (200, 100);
    let camera = must(Camera::for_mesh(&mesh, 2.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, size, "clear-margins");
    assert!(
        frame.is_terrain(100, 50),
        "the centre is terrain, not {:?}",
        frame.at(100, 50)
    );
    let want = expected(CLEAR_COLOR, frame.format);
    for (x, y) in [(0, 0), (0, 99), (199, 0), (199, 99), (5, 50), (194, 50)] {
        assert!(
            near(frame.at(x, y), want),
            "pixel ({x}, {y}) of the square map's margin is {:?}, want {want:?}",
            frame.at(x, y)
        );
    }
}

// ---- REN-021 -----------------------------------------------------------

/// A plane: its map, its height as a function of the world position, and
/// the sampled fractions of the extent.
struct Plane {
    name: &'static str,
    map: Map,
    surface: fn(f64, f64) -> f64,
}

fn planes() -> Vec<Plane> {
    let at = |n: u32| i16::try_from(n).unwrap_or(0);
    vec![
        Plane {
            name: "east",
            map: map_of(8, 64.0, 1.0, |i, _| 4 * at(i)),
            surface: |x, _| 0.5 * x,
        },
        Plane {
            name: "south",
            map: map_of(8, 64.0, 1.0, |_, j| 4 * at(j)),
            surface: |_, z| 0.5 * z,
        },
        Plane {
            name: "diagonal",
            map: map_of(8, 64.0, 1.0, |i, j| 2 * (at(i) + at(j))),
            surface: |x, z| 0.25 * (x + z),
        },
        Plane {
            name: "flat",
            map: map_of(8, 64.0, 1.0, |_, _| 7),
            surface: |_, _| 7.0,
        },
    ]
}

const FRACTIONS: [f64; 5] = [0.08, 0.25, 0.5, 0.75, 0.92];

#[test]
fn conformance_gpu_terrain_pixels_match_the_shading_function() {
    for format in [
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8Unorm,
    ] {
        let Some(mut renderer) = gpu(format) else {
            return;
        };
        for plane in planes() {
            let mesh = scene(&mut renderer, &plane.map, false);
            let size = (192, 192);
            let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
            let label = format!("shading-{format:?}-{}", plane.name);
            let frame = draw_frame(&mut renderer, &camera, size, &label);
            let matrix = camera.view_projection();
            let mut heights = Vec::new();
            for u in FRACTIONS {
                for v in [0.08, 0.5, 0.92] {
                    let ((x, y), color, height) =
                        expected_on_plane(&mesh, &matrix, size, &plane.surface, (u, v));
                    heights.push(height);
                    assert!(
                        frame.is_terrain(x, y),
                        "{label}: ({x}, {y}) is terrain, not {:?}",
                        frame.at(x, y)
                    );
                    let want = expected(color, format);
                    assert!(
                        near(frame.at(x, y), want),
                        "{label}: ({x}, {y}) at height {height} is {:?}, want {want:?}",
                        frame.at(x, y)
                    );
                }
            }
            if plane.name != "flat" {
                let (lo, hi) = heights
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
                assert!(
                    lo < 0.2 && hi > 0.8,
                    "{label}: samples span heights {lo} to {hi}, short of the ramp's ends"
                );
            }
        }
    }
}

// ---- REN-022 -----------------------------------------------------------

#[test]
fn conformance_gpu_water_hides_only_terrain_below_its_surface() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    // Elevation equals x, so the plane at 32 covers the western half.
    let map = with_water(
        map_of(8, 64.0, 1.0, |i, _| 8 * i16::try_from(i).unwrap_or(0)),
        32.0,
    );
    let mesh = scene(&mut renderer, &map, true);
    let size = (192, 192);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, size, "water-depth");
    let matrix = camera.view_projection();

    let water_want = expected(WATER_COLOR, format);
    for u in [0.1, 0.25, 0.4] {
        let (x, y) = pixel_of(&matrix, [u * 64.0, 32.0, 32.0], size);
        let pixel = frame.at(x, y);
        assert!(
            near(pixel, water_want),
            "terrain below the surface at ({x}, {y}) shows {pixel:?}, want water {water_want:?}"
        );
        assert!(
            pixel[2] > pixel[0] && pixel[2] > pixel[1],
            "the water's blue exceeds its red and green: {pixel:?}"
        );
    }
    for u in [0.6, 0.75, 0.9] {
        let ((x, y), color, _) = expected_on_plane(&mesh, &matrix, size, &|x, _| x, (u, 0.5));
        let pixel = frame.at(x, y);
        assert!(
            frame.is_terrain(x, y),
            "terrain above the surface at ({x}, {y}) shows {pixel:?}"
        );
        assert!(
            near(pixel, expected(color, format)),
            "terrain above the surface at ({x}, {y}) is {pixel:?}, want its own colour"
        );
        assert!(
            !(pixel[2] > pixel[0] && pixel[2] > pixel[1]),
            "a terrain colour is not told by blue exceeding red and green: {pixel:?}"
        );
    }
}

// ---- REN-023 -----------------------------------------------------------

/// Higher to the east, with a high shelf along the north edge (rows 0 and
/// 1): elevation sample times 0.125, a relief of 12.5 so the near end still
/// sees the whole slope.
fn shelf_map() -> Map {
    map_of(8, 64.0, 0.125, |i, j| {
        if j < 2 {
            140
        } else {
            40 + 8 * i16::try_from(i).unwrap_or(0)
        }
    })
}

#[test]
fn conformance_gpu_north_is_up_and_east_is_right_at_both_zoom_ends() {
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let mesh = scene(&mut renderer, &shelf_map(), false);
    let size = (160, 160);
    let far = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let mut near_end = far;
    near_end.zoom(-1000.0, ZoomSpeed::Normal);
    near_end.pan(0.0, -8.0);
    for (name, camera) in [("far", far), ("near", near_end)] {
        let frame = draw_frame(&mut renderer, &camera, size, &format!("orientation-{name}"));
        let column = frame.column_span(80);
        assert!(column.is_some(), "{name}: the centre column holds terrain");
        let (top, bottom) = column.unwrap_or((0, 0));
        let (north, south) = (frame.at(80, top + 2), frame.at(80, bottom - 2));
        assert!(
            luminance(north) > luminance(south) + 60,
            "{name}: the shelf is at the top: row {} reads {north:?}, row {} reads {south:?}",
            top + 2,
            bottom - 2
        );
        let row = frame.row_span(80);
        assert!(row.is_some(), "{name}: the centre row holds terrain");
        let (left, right) = row.unwrap_or((0, 0));
        let span = right - left;
        let west = frame.at(left + span * 15 / 100, 80);
        let east = frame.at(right - span * 15 / 100, 80);
        assert!(
            luminance(east) > luminance(west) + 40,
            "{name}: the higher ground is at the right: west reads {west:?}, east reads {east:?}"
        );
    }
}

// ---- REN-024 to REN-027 ------------------------------------------------

#[test]
fn render_grid_fit_is_a_pure_function_of_cells_and_limits() {
    assert!(grid_fits(2048, 2048, DEFAULT_MAX_BUFFER).is_ok());
    assert!(matches!(
        grid_fits(4096, 4096, DEFAULT_MAX_BUFFER),
        Err(Error::GridTooLarge {
            columns: 4096,
            rows: 4096,
            buffer: "vertex",
            limit: DEFAULT_MAX_BUFFER,
            ..
        })
    ));
    // One cell: four vertices of 24 bytes, six indices of 4.
    assert!(grid_fits(1, 1, 96).is_ok());
    assert!(matches!(
        grid_fits(1, 1, 95),
        Err(Error::GridTooLarge { bytes: 96, .. })
    ));
    assert!(grid_fits(1, 1, u64::MAX).is_ok());
    assert!(
        grid_fits(u32::MAX, u32::MAX, u64::MAX).is_err(),
        "the byte count is computed without overflow"
    );
}

#[test]
fn conformance_gpu_upload_refuses_a_grid_over_the_device_limit() {
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let limit = renderer.device().limits().max_buffer_size;
    assert!(
        limit <= DEFAULT_MAX_BUFFER,
        "the device is requested with the default limit, not the adapter's {limit}"
    );
    let mut mesh = mesh_of(&east_ramp());
    mesh.columns = 4096;
    mesh.rows = 4096;
    let refused = renderer.upload(&mesh, None);
    assert!(
        matches!(refused, Err(Error::GridTooLarge { columns: 4096, .. })),
        "a 4096-cell grid is refused before any buffer exists: {refused:?}"
    );
    let mut small = mesh_of(&east_ramp());
    small.positions.truncate(10);
    let inconsistent = renderer.upload(&small, None);
    assert!(
        matches!(inconsistent, Err(Error::InconsistentMesh)),
        "buffers that do not match the cell counts are refused: {inconsistent:?}"
    );
}

#[test]
fn render_missing_adapter_is_an_error_value() {
    let made = Renderer::with_backends(Backends::empty(), TextureFormat::Rgba8UnormSrgb);
    assert!(
        matches!(made, Err(Error::NoAdapter { .. })),
        "an empty set of backends has no adapter: {:?}",
        made.as_ref().err()
    );
    let text = made
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        text.contains("adapter"),
        "the error names what failed: {text}"
    );
}

#[test]
fn conformance_gpu_misuse_is_an_error_value() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    let mesh = mesh_of(&east_ramp());
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let texture = renderer.device().create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let before = renderer.draw(&camera, &view, (4, 4));
    assert!(
        matches!(before, Err(Error::NothingUploaded)),
        "a draw before any upload is refused: {before:?}"
    );
    must(renderer.upload(&mesh, None), "the mesh uploads");
    must(
        renderer.draw(&camera, &view, (4, 4)),
        "a well-formed draw succeeds",
    );
    for size in [(0, 4), (4, 0), (0, 0)] {
        let zero = renderer.draw(&camera, &view, size);
        assert!(
            matches!(zero, Err(Error::ZeroTarget { .. })),
            "a target of {size:?} is refused: {zero:?}"
        );
    }
}

/// Calls made inside the renderer's error scopes come back as values, and
/// the renderer draws afterwards.
#[test]
fn conformance_gpu_guarded_calls_return_errors_as_values() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    let ran = std::cell::Cell::new(false);
    let valid = renderer.guarded("a valid call", || {
        ran.set(true);
        7_u32
    });
    assert!(ran.get(), "the caller's calls are run");
    assert_eq!(valid, Ok(7), "a call that raises nothing returns its value");

    let device = renderer.device().clone();
    let invalid = renderer.guarded("a zero-width texture", || {
        let _ = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 0,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    });
    assert!(
        matches!(
            &invalid,
            Err(Error::Gpu {
                during: "a zero-width texture",
                ..
            })
        ),
        "an invalid call is an error value: {invalid:?}"
    );

    let mesh = mesh_of(&east_ramp());
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    must(renderer.upload(&mesh, None), "the mesh uploads");
    let texture = device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    must(
        renderer.draw(&camera, &view, (4, 4)),
        "the renderer still draws after a captured error",
    );
}

// ---- REN-028 -----------------------------------------------------------

#[test]
fn render_gpu_gate_decides_over_four_cases() {
    assert_eq!(gpu_gate(true, false), GpuGate::Run);
    assert_eq!(gpu_gate(true, true), GpuGate::Run);
    let skip = gpu_gate(false, false);
    let required = gpu_gate(false, true);
    assert!(
        matches!(&skip, GpuGate::Skip(reason) if reason.contains("adapter")),
        "no adapter and none required skips with a reason naming the adapter: {skip:?}"
    );
    assert!(
        matches!(&required, GpuGate::Fail(reason) if reason.contains("adapter")),
        "no adapter with SUPERCOM_REQUIRE_GPU=1 fails: {required:?}"
    );
    assert!(
        matches!((&skip, &required), (GpuGate::Skip(a), GpuGate::Fail(b)) if a == b),
        "both outcomes carry one reason"
    );
}

// ---- REN-029 -----------------------------------------------------------

#[test]
fn render_shader_source_is_assembled_from_the_rust_constants() {
    let source = shader_source();
    for entry in ["vs_terrain", "fs_terrain", "vs_water", "fs_water"] {
        assert!(source.contains(entry), "the source has {entry}");
    }
    let numbers = LIGHT_DIRECTION
        .iter()
        .chain(RAMP.iter().flatten())
        .chain([&AMBIENT]);
    for number in numbers {
        assert!(
            source.contains(&format!("{number:?}")),
            "the source writes {number:?} from the Rust constant"
        );
    }
}

// ---- REN-030, REN-031 --------------------------------------------------

/// Reads the one `.scmap` file of an installed map folder.
fn load(fa: &Path, folder: &str) -> Map {
    let dir = fa.join("maps").join(folder);
    let entries = must(fs::read_dir(&dir), "the map folder lists");
    let file = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "scmap"));
    let bytes = must(
        fs::read(file.unwrap_or_else(|| dir.join("missing.scmap"))),
        "the map file reads",
    );
    must(Map::parse(&bytes), "the installed map parses")
}

#[test]
fn conformance_gpu_installed_dry_map_shows_only_terrain() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let map = load(&fa, "SCMP_012");
    let mesh = scene(&mut renderer, &map, true);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, (512, 512), "SCMP_012");
    let (lo, hi) = (512 / 10, 512 * 9 / 10);
    for y in lo..hi {
        for x in lo..hi {
            assert!(
                frame.is_terrain(x, y),
                "({x}, {y}) of the central region is {:?}, neither clear nor water",
                frame.at(x, y)
            );
        }
    }
    assert_eq!(
        frame.count(Frame::is_water),
        0,
        "a map without water shows none"
    );
}

#[test]
fn conformance_gpu_installed_wet_map_shows_its_water_share() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let map = load(&fa, "SCMP_016");
    let mesh = scene(&mut renderer, &map, true);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, (512, 512), "SCMP_016");
    let covered = frame.count(|f, x, y| !f.is_clear(x, y));
    let water = frame.count(Frame::is_water);
    assert!(covered > 0, "the map covers part of the frame");
    #[expect(
        clippy::cast_precision_loss,
        reason = "pixel counts are at most 262,144, exact in f64"
    )]
    let share = water as f64 / covered as f64;
    assert!(
        (share - 0.095).abs() <= 0.03,
        "water covers {share} of the {covered} map pixels; the heightmap puts 0.095 of its samples at or below the surface"
    );
}

// ---- sentences of REN-021, REN-028, and REN-029 pinned further --------

/// Pins REN-021's sentence that a pixel holds the shading function's colour
/// where the direct term is clamped at zero: a plane steep enough that its
/// normal faces away from the light shows the ramp colour times the ambient
/// share alone.
#[test]
fn conformance_gpu_surface_facing_away_from_the_light_is_ambient_only() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    let at = |n: u32| i16::try_from(n).unwrap_or(0);
    // Falls 1.25 world units per unit toward the south-east, away from the
    // light, which comes from the north-west.
    let map = map_of(8, 64.0, 1.0, |i, j| 10 * (16 - at(i) - at(j)));
    let mesh = scene(&mut renderer, &map, false);
    let normal = mesh.normals[mesh.normals.len() / 2];
    let facing: f32 = normal.iter().zip(LIGHT_DIRECTION).map(|(n, l)| n * l).sum();
    assert!(
        facing < 0.0,
        "the plane faces away from the light: {facing}"
    );
    let size = (192, 192);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, size, "ambient-only");
    let matrix = camera.view_projection();
    let surface = |x: f64, z: f64| 160.0 - 1.25 * (x + z);
    for across in FRACTIONS {
        for down in [0.08, 0.5, 0.92] {
            let ((x, y), color, height) =
                expected_on_plane(&mesh, &matrix, size, &surface, (across, down));
            assert!(
                frame.is_terrain(x, y),
                "({x}, {y}) is terrain, not {:?}",
                frame.at(x, y)
            );
            let want = expected(color, format);
            assert!(
                near(frame.at(x, y), want),
                "({x}, {y}) at height {height} is {:?}, want ambient only {want:?}",
                frame.at(x, y)
            );
        }
    }
}

/// The mesh's own surface at a world position: the elevation and the
/// vertex normals interpolated inside the triangle that holds it, the
/// smallest barycentric weight, and whether the triangle's normals differ.
struct MeshPoint {
    elevation: f64,
    normal: [f64; 3],
    min_weight: f64,
    creased: bool,
}

fn mesh_point(mesh: &TerrainMesh, east: f64, south: f64) -> MeshPoint {
    let (fx, fz) = (
        east / (f64::from(mesh.width) / f64::from(mesh.columns)),
        south / (f64::from(mesh.height) / f64::from(mesh.rows)),
    );
    let (column, row) = (pixel_index(fx, mesh.columns), pixel_index(fz, mesh.rows));
    let (across, down) = (fx - f64::from(column), fz - f64::from(row));
    let north_west = to_usize(row * (mesh.columns + 1) + column);
    let north_east = north_west + 1;
    let south_west = north_west + to_usize(mesh.columns + 1);
    let south_east = south_west + 1;
    // The cell splits from its north-east vertex to its south-west one.
    let corners = if across + down <= 1.0 {
        [
            (north_west, 1.0 - across - down),
            (north_east, across),
            (south_west, down),
        ]
    } else {
        [
            (north_east, 1.0 - down),
            (south_west, 1.0 - across),
            (south_east, across + down - 1.0),
        ]
    };
    let mut elevation = 0.0;
    let mut normal = [0.0; 3];
    for (vertex, weight) in corners {
        elevation += weight * f64::from(mesh.positions[vertex][1]);
        for (sum, component) in normal.iter_mut().zip(mesh.normals[vertex]) {
            *sum += weight * f64::from(component);
        }
    }
    let first = mesh.normals[corners[0].0];
    let creased = corners.iter().any(|&(vertex, _)| {
        mesh.normals[vertex]
            .iter()
            .zip(first)
            .any(|(p, q)| (p - q).abs() > 1e-4)
    });
    MeshPoint {
        elevation,
        normal,
        min_weight: corners.iter().map(|c| c.1).fold(f64::MAX, f64::min),
        creased,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the shading function takes f32"
)]
fn as_f32(v: [f64; 3]) -> [f32; 3] {
    v.map(|c| c as f32)
}

/// Pins REN-021's words "the interpolated normal renormalized": on a tent
/// with a crease, neighbouring vertices carry different normals, and the
/// pixels inside such triangles hold the shading function's colour for the
/// interpolated normal scaled back to unit length.
#[test]
fn conformance_gpu_interpolated_normal_is_renormalized() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    let map = map_of(8, 64.0, 1.0, |i, _| {
        8 * i16::try_from(i.min(8 - i)).unwrap_or(0)
    });
    let mesh = scene(&mut renderer, &map, false);
    let size = (192, 192);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let frame = draw_frame(&mut renderer, &camera, size, "renormalized-normal");
    let matrix = camera.view_projection();
    let surface = |x: f64, z: f64| mesh_point(&mesh, x, z).elevation;
    let mut largest_gap = 0.0_f64;
    let mut checked = 0;
    for x in [26.0, 28.0, 30.0, 34.0, 36.0, 38.0] {
        for z in [12.0, 28.0, 44.0] {
            let nominal = [x, surface(x, z), z];
            let (px, py) = pixel_of(&matrix, nominal, size);
            let (sx, sz) = surface_under(&matrix, centre_ndc(px, py, size), &surface, (x, z));
            let point = mesh_point(&mesh, sx, sz);
            if !point.creased || point.min_weight < 0.1 {
                continue;
            }
            checked += 1;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the shading function takes f32"
            )]
            let height = normalize_height(point.elevation as f32, mesh.lowest, mesh.highest);
            let length = point.normal.iter().map(|c| c * c).sum::<f64>().sqrt();
            let unit = point.normal.map(|c| c / length);
            let want = expected(shade(height, as_f32(unit)), format);
            let raw = expected(shade(height, as_f32(point.normal)), format);
            largest_gap = largest_gap.max(
                want.iter()
                    .zip(raw)
                    .map(|(w, r)| (w - r).abs())
                    .fold(0.0, f64::max),
            );
            assert!(
                frame.is_terrain(px, py),
                "({px}, {py}) is terrain, not {:?}",
                frame.at(px, py)
            );
            assert!(
                near(frame.at(px, py), want),
                "({px}, {py}) is {:?}, want {want:?} for the renormalized normal",
                frame.at(px, py)
            );
        }
    }
    assert!(
        checked >= 6,
        "{checked} pixels lie inside creased triangles"
    );
    assert!(
        largest_gap > TOLERANCE,
        "without renormalizing the expectation moves only {largest_gap}, so the test could not tell"
    );
}

/// Pins REN-029: the source holds each constant as a whole declaration, so
/// a number that only begins with the Rust constant's text does not pass.
#[test]
fn render_shader_source_holds_each_constant_as_a_whole_token() {
    let source = shader_source();
    let vec3 = |v: [f32; 3]| format!("vec3<f32>({:?}, {:?}, {:?})", v[0], v[1], v[2]);
    let declarations = [
        format!("const AMBIENT: f32 = {AMBIENT:?};"),
        format!("const LIGHT: vec3<f32> = {};", vec3(LIGHT_DIRECTION)),
        format!("const STOP_0: vec3<f32> = {};", vec3(RAMP[0])),
        format!("const STOP_1: vec3<f32> = {};", vec3(RAMP[1])),
        format!("const STOP_2: vec3<f32> = {};", vec3(RAMP[2])),
        format!("const WATER: vec3<f32> = {};", vec3(WATER_COLOR)),
    ];
    for declaration in declarations {
        assert!(
            source.contains(&declaration),
            "the source lacks `{declaration}`"
        );
    }
}

/// Pins REN-028's words "the same reason": with no adapter, skipping and
/// failing carry one reason, and it names the flag that makes the difference.
#[test]
fn render_gpu_gate_reasons_agree_and_name_the_flag() {
    let (GpuGate::Skip(skip), GpuGate::Fail(required)) =
        (gpu_gate(false, false), gpu_gate(false, true))
    else {
        fail("no adapter skips unless required, then fails");
    };
    assert_eq!(skip, required, "both outcomes carry one reason");
    assert!(
        skip.contains("SUPERCOM_REQUIRE_GPU=1"),
        "the reason names the flag: {skip}"
    );
}

/// Pins the record's "Errors are values" entry and the errors `draw`
/// documents: a size that does not match the view is a validation error
/// wgpu reports, captured by the frame's error scope and returned as a
/// value, and the renderer draws correctly afterwards.
#[test]
fn conformance_gpu_mismatched_target_size_is_an_error_value() {
    let format = TextureFormat::Rgba8UnormSrgb;
    let Some(mut renderer) = gpu(format) else {
        return;
    };
    let mesh = scene(&mut renderer, &east_ramp(), false);
    let camera = must(Camera::for_mesh(&mesh, 1.0), "the camera builds");
    let texture = renderer.device().create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let mismatched = renderer.draw(&camera, &view, (8, 8));
    assert!(
        matches!(
            mismatched,
            Err(Error::Gpu {
                during: "frame submission",
                ..
            })
        ),
        "a 4 by 4 view drawn as 8 by 8 is a captured validation error: {mismatched:?}"
    );
    let after = renderer.draw(&camera, &view, (4, 4));
    assert!(after.is_ok(), "a correct draw follows: {after:?}");
}

/// Pins REN-025's sentence that a mesh whose buffers do not match its cell
/// counts is refused: one normal too few, and one cell's indices too few.
#[test]
fn conformance_gpu_upload_refuses_mismatched_normals_and_indices() {
    let Some(mut renderer) = gpu(TextureFormat::Rgba8UnormSrgb) else {
        return;
    };
    let mut short_normals = mesh_of(&east_ramp());
    short_normals.normals.pop();
    let refused = renderer.upload(&short_normals, None);
    assert!(
        matches!(refused, Err(Error::InconsistentMesh)),
        "a mesh with one normal too few is refused: {refused:?}"
    );
    let mut short_indices = mesh_of(&east_ramp());
    short_indices
        .indices
        .truncate(short_indices.indices.len() - 6);
    let refused = renderer.upload(&short_indices, None);
    assert!(
        matches!(refused, Err(Error::InconsistentMesh)),
        "a mesh with one cell's indices missing is refused: {refused:?}"
    );
}
