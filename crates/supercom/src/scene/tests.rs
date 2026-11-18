use super::*;
use crate::markers::{COMMANDER_MODEL, MOBILE_MODEL, PROJECTILE_MODEL, STATIC_MODEL};
use crate::units::{Meshes, UnitMesh};
use std::sync::Arc;
use supercom_render::{Camera, Model, Style};
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

fn small_model(lowest: f32) -> Model {
    Model {
        positions: vec![[0.0, lowest, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    }
}

/// Two resolved kinds, in sorted order: the commander's kind at scale 0.25
/// and the mobile kind, whose model reaches 10.284 below its origin, at 0.5.
fn unit_meshes() -> Meshes {
    Meshes::new(vec![
        UnitMesh {
            kind: "commander1".to_owned(),
            scale: 0.25,
            model: small_model(0.0),
        },
        UnitMesh {
            kind: "mobile1".to_owned(),
            scale: 0.5,
            model: small_model(-10.284),
        },
    ])
}

/// The world point a model-space point is placed at by a column-major
/// transform.
fn apply(t: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| t[0][r] * v[0] + t[1][r] * v[1] + t[2][r] * v[2] + t[3][r])
}

fn drawn(state: &State, kinds: &BTreeMap<String, UnitKind>, distance: f32) -> Vec<Instance> {
    let list = scene_instances(
        state,
        &Kept::default(),
        0.0,
        kinds,
        &unit_meshes(),
        &ramp_mesh(),
        distance,
    );
    assert!(list.is_ok(), "{:?}", list.as_ref().err());
    list.unwrap_or_default()
}

/// The rule is a pure function of the camera's distance and the unit's own
/// size: a mesh within 50 times its own size, a marker beyond, never both.
#[test]
fn supercom_mesh_or_marker_rule_cases() {
    for (distance, own, want) in [
        (8.0, 1.0, true),
        (49.0, 1.0, true),
        (51.0, 1.0, false),
        (309.02, 1.0, false),
        (8.0, 3.0, true),
        (149.0, 3.0, true),
        (151.0, 3.0, false),
        (309.02, 8.0, true),
        (f32::NAN, 1.0, false),
        (f32::INFINITY, 1.0, false),
        (8.0, 0.0, false),
        (8.0, f32::NAN, false),
    ] {
        assert_eq!(
            draws_mesh(distance, own),
            want,
            "distance {distance}, size {own}"
        );
    }

    // Over a sweep of distances every unit is exactly one instance, a lit
    // mesh or a flat marker.
    let kinds = kinds();
    let mut state = state_of(&kinds);
    state.spawn(Army(1), "mobile1", pos(8.0, 0.0, 8.0), hundred());
    state.spawn(Army(2), "commander1", pos(32.0, 0.0, 32.0), hundred());
    state.spawn(Army(3), "static1", pos(48.0, 0.0, 16.0), hundred());
    for distance in [8.0, 20.0, 49.0, 51.0, 120.0, 151.0, 309.02] {
        let list = drawn(&state, &kinds, distance);
        assert_eq!(list.len(), 3, "one instance per unit at {distance}");
        for instance in &list {
            assert_eq!(
                instance.style == Style::Lit,
                instance.model >= 4,
                "a lit instance is a mesh and a flat one a marker at {distance}"
            );
        }
        assert_eq!(
            list[0].style == Style::Lit,
            distance <= 50.0,
            "mobile at {distance}"
        );
        assert_eq!(
            list[1].style == Style::Lit,
            distance <= 150.0,
            "commander at {distance}"
        );
        assert_eq!(
            list[2].style,
            Style::Flat,
            "a kind not resolved is a marker"
        );
    }
}

/// A resolved unit near the camera is one lit instance of its mesh in its
/// army's colour, dimmed as its marker is when unfinished; far away, or of a
/// kind not resolved, it is its marker; a projectile is always its marker.
#[test]
fn supercom_scene_draws_meshes_and_markers() {
    let kinds = kinds();
    let mut state = state_of(&kinds);
    let mobile = state.spawn(Army(1), "mobile1", pos(8.0, 0.0, 8.0), hundred());
    state.spawn(Army(2), "commander1", pos(32.0, 0.0, 32.0), hundred());
    state.spawn(Army(3), "static1", pos(48.0, 0.0, 16.0), hundred());
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
    let near = drawn(&state, &kinds, 8.0);
    assert_eq!(near.len(), 4, "three units and a projectile");
    // models 4 and 5 are commander1 and mobile1 in sorted order
    assert_eq!((near[0].model, near[0].style), (5, Style::Lit));
    assert_eq!((near[1].model, near[1].style), (4, Style::Lit));
    assert_eq!((near[2].model, near[2].style), (STATIC_MODEL, Style::Flat));
    assert_eq!(
        (near[3].model, near[3].style),
        (PROJECTILE_MODEL, Style::Flat)
    );
    assert_eq!(near[0].color, colour_table()[0]);
    assert_eq!(near[1].color, colour_table()[1]);
    // The mesh is placed at the unit's drawn position (the ramp's height at
    // x 8 is 5) with the blueprint's scale.
    assert!(
        close3(at(&near[0]), [8.0, 5.0, 8.0]),
        "{:?}",
        near[0].transform[3]
    );
    assert!(
        close(near[0].transform[1][1], 0.5),
        "{:?}",
        near[0].transform
    );
    assert!(
        close(near[1].transform[1][1], 0.25),
        "{:?}",
        near[1].transform
    );

    let far = drawn(&state, &kinds, 309.02);
    let shapes: Vec<(usize, Style)> = far.iter().map(|i| (i.model, i.style)).collect();
    assert_eq!(
        shapes,
        [
            (MOBILE_MODEL, Style::Flat),
            (COMMANDER_MODEL, Style::Flat),
            (STATIC_MODEL, Style::Flat),
            (PROJECTILE_MODEL, Style::Flat)
        ],
        "everything is a marker at the strategic view but the commander's mesh"
    );

    // Unfinished: half built, dimmed as the marker is.
    if let Some(entity) = state.entities.get_mut(&mobile) {
        entity.complete = false;
        entity.built = Q::from_num(5);
    }
    let half = drawn(&state, &kinds, 8.0);
    assert_eq!(half[0].style, Style::Lit);
    assert!(
        close3(half[0].color, [0.63, 0.07, 0.07]),
        "{:?}",
        half[0].color
    );
}

/// The one forward yaw: a quarter turn that takes the mesh's +z to the
/// heading, by a proper rotation and a positive scale, whatever the heading;
/// the stored coordinates are placed as they are, so a vertex below the
/// origin is drawn below it.
#[test]
fn supercom_unit_mesh_placement_cases() {
    let kinds = kinds();
    let mut state = state_of(&kinds);
    let id = state.spawn(Army(1), "mobile1", pos(8.0, 0.0, 8.0), hundred());
    for heading in [
        0.0,
        0.7,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        -2.0,
        5.0,
    ] {
        if let Some(entity) = state.entities.get_mut(&id) {
            entity.heading = heading;
        }
        let list = drawn(&state, &kinds, 8.0);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].style, Style::Lit, "heading {heading}");
        let t = list[0].transform;
        let origin = apply(&t, [0.0; 3]);
        let forward = apply(&t, [0.0, 0.0, 1.0]);
        let (sin, cos) = heading.sin_cos();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a test's expected value, f32 as the matrix"
        )]
        let want = [0.5 * cos as f32, 0.0, 0.5 * sin as f32];
        assert!(
            close3(
                [
                    forward[0] - origin[0],
                    forward[1] - origin[1],
                    forward[2] - origin[2]
                ],
                want
            ),
            "heading {heading}: +z goes to {forward:?} from {origin:?}"
        );
        // columns are orthogonal and as long as the scale, and the
        // determinant is positive: a rotation, not a mirror.
        let columns = [t[0], t[1], t[2]].map(|c| [c[0], c[1], c[2]]);
        let dot3 = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (i, a) in columns.iter().enumerate() {
            assert!(close(dot3(*a, *a).sqrt(), 0.5), "column {i} of {t:?}");
            for b in &columns[i + 1..] {
                assert!(dot3(*a, *b).abs() < 1e-4, "{t:?}");
            }
        }
        let det = dot3(columns[0], cross(columns[1], columns[2]));
        assert!(det > 0.0, "determinant {det} at heading {heading}");
        // the vertex 10.284 below the origin is drawn 10.284 times the scale
        // below it.
        let low = apply(&t, [0.0, -10.284, 0.0]);
        assert!(close(low[1], 5.0 - 10.284 * 0.5), "{low:?}");
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The kinds met that no resolved kind covers are counted once each by the
/// blueprint name the entity carries, projectiles never, and a unit of such a
/// kind is drawn as its marker at the near end of the zoom.
#[test]
fn supercom_unnamed_kinds_are_markers_and_counted() {
    let kinds = kinds();
    let meshes = unit_meshes();
    let mut state = state_of(&kinds);
    state.spawn(Army(1), "mobile1", pos(8.0, 0.0, 8.0), hundred());
    assert!(
        meshes.unnamed(&state).is_empty(),
        "a resolved kind is named"
    );
    state.spawn(Army(2), "static1", pos(32.0, 0.0, 32.0), hundred());
    state.spawn(Army(2), "static1", pos(40.0, 0.0, 32.0), hundred());
    state.spawn(Army(3), "nosuchkind", pos(48.0, 0.0, 16.0), hundred());
    let want: std::collections::BTreeSet<String> =
        ["static1", "nosuchkind"].map(str::to_owned).into();
    assert_eq!(meshes.unnamed(&state), want, "distinct, by blueprint name");
    let near = drawn(&state, &kinds, 8.0);
    assert_eq!(near.len(), 4);
    assert_eq!(near[0].style, Style::Lit);
    assert!(
        near[1..]
            .iter()
            .all(|i| i.style == Style::Flat && i.model < 4),
        "an unnamed kind is a marker at the near end"
    );
    assert!(
        Meshes::default().unnamed(&state).len() == 3,
        "with no kind named, all three"
    );
}

/// The linear value of an sRGB-encoded 0-to-255 channel.
fn decode_srgb(byte: u8) -> f64 {
    let v = f64::from(byte) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// For `pixel`, the light factor L in 0 to 1 when the pixel is `colour`
/// times L within 0.02 per linear channel, else `None`.
fn light_of(pixel: [u8; 3], colour: [f32; 3]) -> Option<f64> {
    let lin = pixel.map(decode_srgb);
    let strongest = (0..3).max_by(|a, b| colour[*a].total_cmp(&colour[*b]))?;
    let light = lin[strongest] / f64::from(colour[strongest]);
    let fits = (0..3).all(|k| (lin[k] - f64::from(colour[k]) * light).abs() <= 0.02);
    (fits && (0.0..=1.02).contains(&light)).then_some(light)
}

/// The lights of the pixels in a square window of `half` pixels around
/// `(x, y)` that read as `colour` times a light.
fn lights_around(
    pixels: &[[u8; 3]],
    size: u32,
    (x, y): (u32, u32),
    half: u32,
    colour: [f32; 3],
) -> Vec<f64> {
    let mut lights = Vec::new();
    for row in y.saturating_sub(half)..(y + half).min(size) {
        for column in x.saturating_sub(half)..(x + half).min(size) {
            let index = usize::try_from(row * size + column).unwrap_or(0);
            if let Some(light) = pixels.get(index).and_then(|p| light_of(*p, colour)) {
                lights.push(light);
            }
        }
    }
    lights
}

/// Whole path at the near end of the zoom: over the first commander of the
/// committed log's start state, the frame shows its mesh lit in its army's
/// colour: pixels of that colour times a light that varies across the mesh
/// from the ambient share to full; the same frame with the commander drawn
/// flat, or not at all, or in the other army's colour, shows no such thing.
#[test]
fn conformance_gpu_close_zoom_draws_the_commander_mesh_lit_in_its_army_colour() {
    use crate::prepare::prepare_replay;
    use supercom_render::wgpu::TextureFormat;
    use supercom_render::{GpuGate, Renderer, WaterPlane, ZoomSpeed, gpu_gate};

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
    let scratch = crate::testing::Scratch::new("close-zoom");
    let prepared = prepare_replay(
        Some(&fa),
        &crate::testing::committed_log(),
        scratch.cache_vars(),
    );
    assert!(prepared.is_ok(), "{:?}", prepared.as_ref().err());
    let Ok(prepared) = prepared else { return };

    let terrain = TerrainMesh::build(&prepared.map).expect("the map builds");
    let mut camera = Camera::for_mesh(&terrain, 1.0).expect("the camera");
    camera.zoom(-10_000.0, ZoomSpeed::Fast);
    assert!(
        close(camera.distance(), 8.0),
        "the near end: {}",
        camera.distance()
    );
    let list = scene_instances(
        &prepared.state,
        &Kept::default(),
        0.0,
        &prepared.state.world().units,
        &prepared.meshes,
        &terrain,
        camera.distance(),
    );
    assert!(list.is_ok(), "{:?}", list.as_ref().err());
    let list = list.unwrap_or_default();
    assert_eq!(list.len(), 2, "two commanders stand at the start");
    assert_eq!(
        list.iter().map(|i| i.style).collect::<Vec<_>>(),
        [Style::Lit, Style::Lit],
        "both are meshes at the near end"
    );

    // Centre the camera on the first commander: the line of sight through
    // the target reaches the commander's height south of the target by the
    // drop over the tangent of the pitch.
    let commander = at(&list[0]);
    let drop = camera.target()[1] - commander[1];
    let target = camera.target();
    camera.pan(
        commander[0] - target[0],
        commander[2] - target[2] + drop / camera.pitch().tan(),
    );

    let size = 512;
    let water = WaterPlane::from_map(&prepared.map);
    let mut renderer = made.expect("the renderer is built on the adapter found");
    renderer
        .upload(&terrain, water.as_ref())
        .expect("the terrain uploads");
    renderer
        .upload_models(&prepared.meshes.model_set())
        .expect("the models upload");
    let matrix = camera.view_projection();
    let centre = pixel_of(
        &matrix,
        [commander[0], commander[1] + 0.8, commander[2]],
        size,
    );
    let probe = Probe {
        size,
        centre,
        colour: army_colour(Army(1)),
    };
    assert_lit_in_army_colour(&mut renderer, &camera, &list, &probe);
}

/// The assertions on the close-zoom frame: the commander is lit, and the
/// controls (flat, absent, recoloured) show what the detector would miss.
fn assert_lit_in_army_colour(
    renderer: &mut supercom_render::Renderer,
    camera: &Camera,
    list: &[Instance],
    probe: &Probe,
) {
    let lights = probe.lights(renderer, camera, list);
    assert!(
        lights.len() >= 150,
        "the commander's pixels in its army's colour: {} near {:?}",
        lights.len(),
        probe.centre
    );
    let (low, high) = lights
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), l| {
            (lo.min(*l), hi.max(*l))
        });
    assert!(low >= 0.28, "no pixel darker than the ambient share: {low}");
    assert!(
        high - low >= 0.15,
        "the light varies across the mesh: {low} to {high}"
    );
    assert!(
        lights.iter().filter(|l| **l < 0.95).count() >= 100,
        "shaded pixels, not a flat colour"
    );
    let other = Probe {
        colour: army_colour(Army(2)),
        ..*probe
    };
    let strangers = other.lights(renderer, camera, list);
    assert!(
        strangers.len() * 20 < lights.len(),
        "the other army's colour: {}",
        strangers.len()
    );

    // The controls: flat, absent, and in the other army's colour.
    let mut flat = list.to_vec();
    flat[0].style = Style::Flat;
    let flat_lights = probe.lights(renderer, camera, &flat);
    assert!(
        flat_lights.iter().filter(|l| **l < 0.95).count() < 20,
        "a flat commander has no shading: {}",
        flat_lights.len()
    );
    let absent = probe.lights(renderer, camera, &list[1..]);
    assert!(
        absent.len() < 20,
        "no commander, no pixels: {}",
        absent.len()
    );
    let mut recoloured = list.to_vec();
    recoloured[0].color = army_colour(Army(2));
    let wrong = probe.lights(renderer, camera, &recoloured);
    assert!(wrong.len() < 20, "the other army's colour: {}", wrong.len());
}

/// What a frame is read for: the target's size, the pixel the commander is
/// expected at, and the army colour looked for.
#[derive(Clone, Copy)]
struct Probe {
    size: u32,
    centre: (u32, u32),
    colour: [f32; 3],
}

impl Probe {
    /// Draws `instances` and returns the lights of the pixels around the
    /// expected pixel that read as the colour times a light.
    fn lights(
        &self,
        renderer: &mut supercom_render::Renderer,
        camera: &Camera,
        instances: &[Instance],
    ) -> Vec<f64> {
        renderer
            .set_instances(instances)
            .expect("the list is accepted");
        let pixels = draw_square(renderer, camera, self.size);
        lights_around(&pixels, self.size, self.centre, 48, self.colour)
    }
}
