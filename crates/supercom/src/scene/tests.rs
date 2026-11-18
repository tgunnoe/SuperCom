use super::*;
use crate::markers::{COMMANDER_MODEL, MOBILE_MODEL, PROJECTILE_MODEL, STATIC_MODEL};
use std::sync::Arc;
use supercom_render::{Camera, Style};
use supercom_scmap::{Heightmap, Map};
use supercom_sim::{Layer, MotionType, Pos, Projectile, ProjectileId, Q, World};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4
}

fn close3(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| close(*a, b))
}

fn at(instance: &Instance) -> [f32; 3] {
    let t = instance.transform[3];
    [t[0], t[1], t[2]]
}

fn pos(x: f64, y: f64, z: f64) -> Pos {
    Pos { x, y, z }
}

/// A 4 by 4 cell map over 64 by 64 world units whose height is 10 per
/// column: the surface rises 10 over every 16 units of x, and 0 along z.
fn ramp_mesh() -> TerrainMesh {
    let mut samples = Vec::new();
    for _row in 0..=4_i16 {
        for column in 0..=4_i16 {
            samples.push(column * 10);
        }
    }
    let map = Map {
        width: 64.0,
        height: 64.0,
        version: 60,
        preview: Vec::new(),
        heightmap: Heightmap {
            width: 4,
            height: 4,
            scale: 1.0,
            samples,
        },
        strata: Vec::new(),
        water: None,
    };
    TerrainMesh::build(&map).expect("the ramp builds")
}

fn kinds() -> BTreeMap<String, UnitKind> {
    let kind = |id: &str, motion: MotionType, footprint: Option<(u32, u32)>, command: bool| {
        let mut kind = UnitKind {
            id: id.to_owned(),
            motion,
            footprint,
            build_time: Some(Q::from_num(10)),
            ..UnitKind::default()
        };
        if command {
            kind.categories.insert("COMMAND".to_owned());
        }
        (id.to_owned(), kind)
    };
    BTreeMap::from([
        kind("mobile1", MotionType::Land, None, false),
        kind("commander1", MotionType::Land, Some((3, 3)), true),
        kind("static1", MotionType::None, Some((3, 2)), false),
        kind("fixedcmd", MotionType::None, Some((1, 1)), true),
    ])
}

fn state_of(kinds: &BTreeMap<String, UnitKind>) -> State {
    let mut world = World::empty();
    world.units = kinds.clone();
    State::with_world(Arc::new(world), 0)
}

fn colour_table() -> [[f32; 3]; 8] {
    [
        [0.9, 0.1, 0.1],
        [0.1, 0.3, 0.9],
        [0.1, 0.7, 0.2],
        [0.9, 0.8, 0.1],
        [0.6, 0.1, 0.8],
        [0.1, 0.8, 0.8],
        [0.9, 0.5, 0.1],
        [0.9, 0.9, 0.9],
    ]
}

fn hundred() -> Q {
    Q::from_num(100)
}

/// The model, colour, order, transform, and style of each instance, the
/// interpolation of one entity and of none, a removed entity, and an
/// unknown blueprint.
#[test]
fn supercom_scene_instances_cases() {
    let kinds = kinds();
    let mesh = ramp_mesh();
    let mut before = state_of(&kinds);
    before.spawn(Army(1), "mobile1", pos(0.0, 0.0, 8.0), hundred());
    before.spawn(Army(2), "commander1", pos(32.0, 0.0, 32.0), hundred());
    before.spawn(Army(3), "static1", pos(48.0, 0.0, 16.0), hundred());
    before.spawn(Army(4), "nosuchkind", pos(16.0, 0.0, 48.0), hundred());
    before.spawn(Army(5), "static1", pos(40.0, 0.0, 40.0), hundred());
    let kept = Kept::of(&before);

    let mut state = state_of(&kinds);
    let mobile = state.spawn(Army(1), "mobile1", pos(10.0, 0.0, 8.0), hundred());
    state.spawn(Army(2), "commander1", pos(32.0, 0.0, 32.0), hundred());
    state.spawn(Army(3), "static1", pos(48.0, 0.0, 16.0), hundred());
    let unknown = state.spawn(Army(4), "nosuchkind", pos(16.0, 0.0, 48.0), hundred());
    if let Some(entity) = state.entities.get_mut(&unknown) {
        entity.complete = false;
        entity.built = Q::ZERO;
    }
    state.projectiles.insert(
        ProjectileId(1),
        Projectile {
            id: ProjectileId(1),
            army: Army(1),
            source: mobile,
            kind: None,
            pos: pos(20.0, 30.0, 40.0),
            vel: [0.0; 3],
            target: None,
            aim: pos(0.0, 0.0, 0.0),
            damage: Q::ZERO,
            damage_radius: 0.0,
            damage_type: String::new(),
            lifetime: None,
        },
    );

    let list = instances(&state, &kept, 0.5, &kinds, &mesh, 100.0);
    assert!(list.is_ok(), "{:?}", list.as_ref().err());
    let list = list.unwrap_or_default();
    let models: Vec<usize> = list.iter().map(|i| i.model).collect();
    assert_eq!(
        models,
        [
            MOBILE_MODEL,
            COMMANDER_MODEL,
            STATIC_MODEL,
            STATIC_MODEL,
            PROJECTILE_MODEL
        ],
        "entities by id, then the projectile; the removed entity is absent"
    );
    assert!(list.iter().all(|i| i.style == Style::Flat));

    // At distance 100 the share is 2: the mobile unit is 1 across and is
    // drawn 2 across; the commander is 3; the fixed kind is 3 by its
    // footprint; the unknown blueprint is 1 and is drawn 2.
    let scales: Vec<f32> = list.iter().map(|i| i.transform[0][0]).collect();
    for (got, want) in scales.iter().zip([2.0, 3.0, 3.0, 2.0, 0.5]) {
        assert!(close(*got, want), "scales {scales:?}");
    }
    // The mobile unit was kept at x 0 and is at 10: half way is 5, and
    // the ramp's height there is 5 * 10 / 16.
    assert!(
        close3(at(&list[0]), [5.0, 3.125, 8.0]),
        "{:?}",
        list[0].transform[3]
    );
    // Entities with no kept value are drawn where they are, on the ramp.
    assert!(close3(at(&list[1]), [32.0, 20.0, 32.0]));
    assert!(close3(at(&list[2]), [48.0, 30.0, 16.0]));
    // A projectile is drawn at its own y, where it is.
    assert!(close3(at(&list[4]), [20.0, 30.0, 40.0]));
    assert_eq!(list[4].color, PROJECTILE_COLOUR);
    // The unknown blueprint is complete as far as the drawing goes.
    assert_eq!(list[3].color, colour_table()[3]);

    // A heading of a quarter turn turns the model's +x onto +z.
    if let Some(entity) = state.entities.get_mut(&mobile) {
        entity.heading = std::f64::consts::FRAC_PI_2;
    }
    let turned = instances(&state, &Kept::default(), 0.0, &kinds, &mesh, 100.0).unwrap_or_default();
    let column = turned.first().map(|i| i.transform[0]).unwrap_or_default();
    assert!(
        close3([column[0], column[1], column[2]], [0.0, 0.0, 2.0]),
        "{column:?}"
    );
}

/// Each army's colour, the repeat, and the dimming of an unfinished kind.
#[test]
fn supercom_scene_colours_and_dimming() {
    for (n, want) in colour_table().into_iter().enumerate() {
        let army = Army(u8::try_from(n + 1).unwrap_or(1));
        assert_eq!(army_colour(army), want, "army {}", n + 1);
    }
    assert_eq!(army_colour(Army(9)), colour_table()[0]);
    assert_eq!(army_colour(Army(16)), colour_table()[7]);
    assert_eq!(army_colour(Army(0)), colour_table()[0]);

    let red = colour_table()[0];
    assert!(close3(dim(red, 0.0), [0.36, 0.04, 0.04]));
    assert!(close3(dim(red, 0.5), [0.63, 0.07, 0.07]));
    assert_eq!(dim(red, 1.0), red);

    let kinds = kinds();
    let mut state = state_of(&kinds);
    let unfinished = state.spawn(Army(1), "mobile1", pos(10.0, 0.0, 10.0), hundred());
    let finished = state.spawn(Army(1), "mobile1", pos(20.0, 0.0, 10.0), hundred());
    if let Some(entity) = state.entities.get_mut(&unfinished) {
        entity.complete = false;
        entity.built = Q::from_num(5);
    }
    let list =
        instances(&state, &Kept::default(), 0.0, &kinds, &ramp_mesh(), 8.0).unwrap_or_default();
    assert_eq!(list.len(), 2, "{finished:?}");
    assert!(close3(list[0].color, [0.63, 0.07, 0.07]), "half built");
    assert_eq!(list[1].color, red, "complete is not dimmed");
    assert!(
        list.iter()
            .all(|i| i.color.iter().all(|c| (0.0..=1.0).contains(c)))
    );
}

/// A marker is the larger of the unit's size and a share of the distance,
/// at both ends of the zoom range.
#[test]
fn supercom_marker_size_cases() {
    assert!(close(marker_size(1.0, 8.0), 1.0));
    assert!(close(marker_size(2.0, 8.0), 2.0));
    assert!(close(marker_size(1.0, 309.02), 6.1804));
    assert!(close(marker_size(8.0, 309.02), 8.0));
    assert!(close(marker_size(1.0, 40.0), 1.0));
    assert!(close(marker_size(1.0, 100.0), 2.0));

    let footprint = |footprint| UnitKind {
        footprint,
        ..UnitKind::default()
    };
    assert!(close(own_size(&footprint(Some((3, 2)))), 3.0));
    assert!(close(own_size(&footprint(Some((2, 5)))), 5.0));
    assert!(close(own_size(&footprint(None)), 1.0));
    assert!(close(own_size(&footprint(Some((0, 0)))), 1.0));
    assert!(close(PROJECTILE_SIZE, 0.5));

    let kinds = kinds();
    assert_eq!(shape_of(kinds.get("mobile1")), (MOBILE_MODEL, 1.0));
    assert_eq!(shape_of(kinds.get("commander1")), (COMMANDER_MODEL, 3.0));
    assert_eq!(shape_of(kinds.get("static1")), (STATIC_MODEL, 3.0));
    assert_eq!(
        shape_of(kinds.get("fixedcmd")),
        (COMMANDER_MODEL, 1.0),
        "a commander is a commander whether or not it moves"
    );
    assert_eq!(shape_of(None), (STATIC_MODEL, 1.0));
}

/// Land and seabed follow the drawn surface; every other layer, and a
/// point outside the extent, keeps the simulation's y.
#[test]
fn supercom_scene_height_by_layer() {
    let kinds = kinds();
    let mesh = ramp_mesh();
    let mut state = state_of(&kinds);
    let layers = [
        (Layer::Land, 8.0, 5.0),
        (Layer::Seabed, 16.0, 10.0),
        (Layer::Air, 8.0, 77.0),
        (Layer::Water, 8.0, 77.0),
        (Layer::Sub, 8.0, 77.0),
        (Layer::Orbit, 8.0, 77.0),
        (Layer::Land, -5.0, 77.0),
    ];
    for (layer, x, _) in layers {
        let id = state.spawn(Army(1), "mobile1", pos(x, 77.0, 20.0), hundred());
        if let Some(entity) = state.entities.get_mut(&id) {
            entity.layer = layer;
        }
    }
    let list = instances(&state, &Kept::default(), 0.0, &kinds, &mesh, 8.0).unwrap_or_default();
    assert_eq!(list.len(), layers.len());
    for (instance, (layer, x, want)) in list.iter().zip(layers) {
        assert!(
            close(instance.transform[3][1], want),
            "{layer:?} at x {x}: {}",
            instance.transform[3][1]
        );
    }
}

/// The 0-to-255 value a linear channel reads back as in a format with the
/// sRGB transfer function.
fn encode_srgb(linear: f32) -> f64 {
    let l = f64::from(linear).clamp(0.0, 1.0);
    let v = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    v * 255.0
}

/// The pixel a world point falls in, from the column-major
/// view-projection matrix and the target's size.
fn pixel_of(matrix: &[[f32; 4]; 4], point: [f32; 3], size: u32) -> (u32, u32) {
    let row = |r: usize| {
        f64::from(matrix[0][r]) * f64::from(point[0])
            + f64::from(matrix[1][r]) * f64::from(point[1])
            + f64::from(matrix[2][r]) * f64::from(point[2])
            + f64::from(matrix[3][r])
    };
    let (nx, ny) = (row(0) / row(3), row(1) / row(3));
    let to_pixel = |v: f64| -> u32 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to the target before the cast"
        )]
        let n = v.floor().clamp(0.0, f64::from(size - 1)) as u32;
        n
    };
    (
        to_pixel(f64::midpoint(nx, 1.0) * f64::from(size)),
        to_pixel(f64::midpoint(1.0, -ny) * f64::from(size)),
    )
}

/// Draws one frame offscreen into a square target of `size` pixels in
/// `Rgba8UnormSrgb` and returns its pixels row by row, row 0 at the top.
fn draw_square(
    renderer: &mut supercom_render::Renderer,
    camera: &Camera,
    size: u32,
) -> Vec<[u8; 3]> {
    use supercom_render::wgpu::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, Origin3d,
        PollType, TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
        TextureDescriptor, TextureDimension, TextureUsages, TextureViewDescriptor,
    };
    let device = renderer.device().clone();
    let extent = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("offscreen"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: renderer.format(),
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let drawn = renderer.draw(camera, &view, (size, size));
    assert!(
        drawn.is_ok(),
        "the draw accepts a well-formed target: {drawn:?}"
    );
    let row_bytes = (size * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row_bytes) * u64::from(size),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
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
                rows_per_image: Some(size),
            },
        },
        extent,
    );
    renderer.queue().submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.map_async(MapMode::Read, .., move |result| {
        drop(sender.send(result));
    });
    let polled = device.poll(PollType::wait_indefinitely());
    assert!(polled.is_ok(), "the device finishes the copy: {polled:?}");
    let mapped = receiver.recv();
    assert!(
        matches!(mapped, Ok(Ok(()))),
        "the readback buffer maps: {mapped:?}"
    );
    let range = buffer.get_mapped_range(..);
    assert!(range.is_ok(), "the buffer is mapped");
    let mut pixels = Vec::new();
    if let Ok(bytes) = range {
        for y in 0..size {
            let start = usize::try_from(y * row_bytes).unwrap_or(0);
            for x in 0..size {
                let at = start + usize::try_from(x).unwrap_or(0) * 4;
                if let Some(rgb) = bytes.get(at..at + 3) {
                    pixels.push([rgb[0], rgb[1], rgb[2]]);
                }
            }
        }
    }
    pixels
}

/// Whole path: the committed log's start state, the instance list at the
/// strategic view, drawn offscreen; the pixel each commander projects to
/// holds its army's colour, and the two differ.
#[test]
fn conformance_gpu_replay_start_draws_each_commander_in_its_army_colour() {
    use crate::markers::marker_models;
    use crate::prepare::prepare_replay;
    use supercom_render::wgpu::TextureFormat;
    use supercom_render::{GpuGate, Renderer, WaterPlane, gpu_gate};

    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let require = std::env::var("SUPERCOM_REQUIRE_GPU").is_ok_and(|value| value == "1");
    let made = Renderer::new(TextureFormat::Rgba8UnormSrgb);
    let found = !matches!(made, Err(supercom_render::Error::NoAdapter { .. }));
    let gate = gpu_gate(found, require);
    if let GpuGate::Skip(reason) = &gate {
        eprintln!("SKIP: {reason}");
        return;
    }
    assert_eq!(gate, GpuGate::Run, "an adapter is required");
    let scratch = crate::testing::Scratch::new("pixel");
    let prepared = prepare_replay(
        Some(&fa),
        &crate::testing::committed_log(),
        scratch.cache_vars(),
    );
    assert!(
        prepared.is_ok(),
        "the committed log prepares: {:?}",
        prepared.as_ref().err()
    );
    let Ok(prepared) = prepared else { return };

    let mesh = TerrainMesh::build(&prepared.map).expect("the map builds");
    let camera = Camera::for_mesh(&mesh, 1.0).expect("the strategic camera");
    let list = instances(
        &prepared.state,
        &Kept::default(),
        0.0,
        &prepared.state.world().units,
        &mesh,
        camera.distance(),
    );
    assert!(list.is_ok(), "{:?}", list.as_ref().err());
    let list = list.unwrap_or_default();
    assert_eq!(list.len(), 2, "two commanders stand at the start");

    let mut renderer = made.expect("the renderer is built on the adapter found");
    let size = 512;
    let water = WaterPlane::from_map(&prepared.map);
    renderer
        .upload(&mesh, water.as_ref())
        .expect("the terrain uploads");
    renderer
        .upload_models(&marker_models())
        .expect("the models upload");
    renderer.set_instances(&list).expect("the list is accepted");
    let pixels = draw_square(&mut renderer, &camera, size);
    assert_eq!(pixels.len(), 512 * 512);

    let matrix = camera.view_projection();
    let mut seen = Vec::new();
    for (instance, army) in list.iter().zip([Army(1), Army(2)]) {
        let (x, y) = pixel_of(&matrix, at(instance), size);
        let index = usize::try_from(y * size + x).unwrap_or(0);
        let pixel = pixels.get(index).copied().unwrap_or([0; 3]);
        let want = army_colour(army).map(encode_srgb);
        for (got, want) in pixel.iter().zip(want) {
            assert!(
                (f64::from(*got) - want).abs() <= 2.0,
                "army {} at pixel ({x}, {y}) reads {pixel:?}, expected {want:?}",
                army.0
            );
        }
        seen.push(pixel);
    }
    assert_ne!(seen.first(), seen.get(1), "the two armies differ");
}

/// REN-066: a projectile is interpolated by its id: from (20, 30, 40) to
/// (30, 10, 40) at one half is drawn at (25, 20, 40).
#[test]
fn supercom_scene_interpolates_a_projectile_by_its_id() {
    let kinds = kinds();
    let mesh = ramp_mesh();
    let mut before = state_of(&kinds);
    let source = before.spawn(Army(1), "static1", pos(8.0, 0.0, 8.0), hundred());
    before.projectiles.insert(
        ProjectileId(1),
        Projectile {
            id: ProjectileId(1),
            army: Army(1),
            source,
            kind: None,
            pos: pos(20.0, 30.0, 40.0),
            vel: [0.0; 3],
            target: None,
            aim: pos(0.0, 0.0, 0.0),
            damage: Q::ZERO,
            damage_radius: 0.0,
            damage_type: String::new(),
            lifetime: None,
        },
    );
    let kept = Kept::of(&before);
    let mut state = state_of(&kinds);
    let source = state.spawn(Army(1), "static1", pos(8.0, 0.0, 8.0), hundred());
    state.projectiles.insert(
        ProjectileId(1),
        Projectile {
            id: ProjectileId(1),
            army: Army(1),
            source,
            kind: None,
            pos: pos(30.0, 10.0, 40.0),
            vel: [0.0; 3],
            target: None,
            aim: pos(0.0, 0.0, 0.0),
            damage: Q::ZERO,
            damage_radius: 0.0,
            damage_type: String::new(),
            lifetime: None,
        },
    );
    let list = instances(&state, &kept, 0.5, &kinds, &mesh, 100.0);
    assert!(list.is_ok(), "{:?}", list.as_ref().err());
    let at = list.unwrap_or_default().last().map(|i| i.transform[3]);
    assert!(
        matches!(at, Some(t) if (t[0] - 25.0).abs() < 1e-4 && (t[1] - 20.0).abs() < 1e-4 && (t[2] - 40.0).abs() < 1e-4),
        "{at:?}"
    );
}
