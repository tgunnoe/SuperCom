use super::*;
use crate::markers::marker_models;
use std::path::Path;
use std::sync::Arc;
use supercom_sim::{
    Army, Command, Order, Pos, Q, QueueMode, REPLAY_FORMAT_VERSION, ReplayHeader, Tick, World,
};

const FIVE: [&str; 5] = ["ueb1103", "ueb2101", "uel0001", "urb1101", "url0001"];

fn bone() -> supercom_scm::Bone {
    supercom_scm::Bone {
        name: "Root".to_owned(),
        parent: None,
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        rest_pose_inverse: [[0.0; 4]; 4],
    }
}

fn vertex(position: [f32; 3], normal: [f32; 3]) -> supercom_scm::Vertex {
    supercom_scm::Vertex {
        position,
        normal,
        tangent: [7.0, 7.5, 8.0],
        binormal: [8.5, 9.0, 9.5],
        uv0: [0.25, 0.5],
        uv1: [0.75, 1.0],
        bone_indices: [0, 0, 0, 0],
    }
}

fn mesh(vertices: Vec<supercom_scm::Vertex>, indices: Vec<u16>) -> Mesh {
    Mesh {
        bones: vec![bone()],
        vertices,
        indices,
    }
}

fn quad_mesh() -> Mesh {
    mesh(
        vec![
            vertex([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            vertex([1.0, 0.0, 0.0], [0.0, 0.6, 0.8]),
            vertex([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            vertex([1.0, -10.284, 1.0], [0.0, 0.0, 1.0]),
        ],
        vec![0, 1, 2, 2, 1, 3],
    )
}

fn model_of(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> Model {
    Model {
        positions: positions.to_vec(),
        normals: normals.to_vec(),
        indices: indices.to_vec(),
    }
}

/// The model is the stored positions, the stored normals through the field
/// `normal`, and the indices widened; nothing else of the mesh reaches it.
#[test]
fn supercom_unit_mesh_model_cases() {
    let model = model_of_mesh(&quad_mesh());
    assert_eq!(
        model,
        model_of(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, -10.284, 1.0]
            ],
            &[
                [0.0, 1.0, 0.0],
                [0.0, 0.6, 0.8],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0]
            ],
            &[0, 1, 2, 2, 1, 3]
        )
    );

    // The widest index a u16 holds stays itself.
    let wide = mesh(
        vec![vertex([0.0; 3], [0.0, 1.0, 0.0])],
        vec![0, 65_534, 65_535],
    );
    assert_eq!(model_of_mesh(&wide).indices, vec![0, 65_534, 65_535]);

    // Bones, tangents, binormals, texture coordinates, and bone indices are
    // never read: a mesh that differs only in them gives the same model.
    let mut other = quad_mesh();
    other.bones.push(supercom_scm::Bone {
        name: "Second".to_owned(),
        parent: Some(0),
        position: [3.0, 2.0, 1.0],
        rotation: [1.0, 0.0, 0.0, 0.0],
        rest_pose_inverse: [[5.0; 4]; 4],
    });
    for vertex in &mut other.vertices {
        vertex.tangent = [-1.0, -2.0, -3.0];
        vertex.binormal = [4.0, 5.0, 6.0];
        vertex.uv0 = [0.9, 0.8];
        vertex.uv1 = [0.7, 0.6];
        vertex.bone_indices = [1, 1, 1, 1];
    }
    assert_eq!(model_of_mesh(&other), model);
}

fn header() -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: "test".to_owned(),
        map: "SCMP_012".to_owned(),
        map_digest: [0; 32],
        catalog_digest: [0; 32],
        plugins: Vec::new(),
        players: Vec::new(),
        seed: 1,
    }
}

fn command(seq: u32, order: Order) -> Command {
    Command {
        tick: Tick(u64::from(seq)),
        source: 0,
        seq,
        army: Army(1),
        units: Vec::new(),
        order,
        mode: QueueMode::Replace,
    }
}

fn replay_of(orders: Vec<Order>) -> Replay {
    Replay {
        header: header(),
        commands: orders
            .into_iter()
            .zip(0..)
            .map(|(order, seq)| command(seq, order))
            .collect(),
        checkpoints: Vec::new(),
    }
}

fn at() -> Pos {
    Pos {
        x: 1.0,
        y: 0.0,
        z: 1.0,
    }
}

fn state_with(blueprints: &[&str]) -> State {
    let mut state = State::with_world(Arc::new(World::empty()), 1);
    for blueprint in blueprints {
        state.spawn(Army(1), blueprint, at(), Q::from_num(100));
    }
    state
}

/// The kinds a run resolves: the blueprint of every entity of the start
/// state and of every build order, in ASCII lower case, sorted and distinct;
/// no other order names one.
#[test]
fn supercom_unit_kinds_named_cases() {
    let state = state_with(&["uel0001", "uel0001", "url0001"]);
    let replay = replay_of(vec![
        Order::BuildMobile {
            blueprint: "UEB1103".to_owned(),
            at: at(),
        },
        Order::BuildFactory {
            blueprint: "ueb1103".to_owned(),
        },
        Order::BuildMobile {
            blueprint: "urb1101".to_owned(),
            at: at(),
        },
        Order::BuildFactory {
            blueprint: "UEL0001".to_owned(),
        },
        Order::Move { to: at() },
        Order::Stop,
    ]);
    assert_eq!(
        run_kinds(&state, &replay),
        ["ueb1103", "uel0001", "urb1101", "url0001"],
    );
    assert_eq!(
        run_kinds(&state_with(&[]), &replay_of(Vec::new())),
        Vec::<String>::new(),
        "no entity and no build order name no kind"
    );
    assert_eq!(
        run_kinds(&state_with(&["uel0001"]), &replay_of(Vec::new())),
        ["uel0001"]
    );
}

fn unit(kind: &str, scale: f32, model: Model) -> UnitMesh {
    UnitMesh {
        kind: kind.to_owned(),
        scale,
        model,
    }
}

/// The set is the four markers, then one model per resolved kind in the
/// order of the kinds, and a kind's index in the set is four plus its place.
#[test]
fn supercom_unit_model_set_order() {
    let triangle = model_of(
        &[[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
        &[[0.0, 1.0, 0.0]; 3],
        &[0, 1, 2],
    );
    let meshes = Meshes::new(vec![
        unit("ueb1103", 0.06, triangle.clone()),
        unit(
            "uel0001",
            0.105,
            Model {
                indices: vec![2, 1, 0],
                ..triangle.clone()
            },
        ),
    ]);
    let set = meshes.model_set();
    assert_eq!(set.len(), 6);
    assert_eq!(set[..4], marker_models()[..]);
    assert_eq!(set[4], triangle);
    assert_eq!(set[5].indices, vec![2, 1, 0]);
    assert_eq!(meshes.get("ueb1103").map(|(index, _)| index), Some(4));
    assert_eq!(meshes.get("uel0001").map(|(index, _)| index), Some(5));
    assert_eq!(
        meshes.get("uel0001").map(|(_, unit)| unit.scale),
        Some(0.105)
    );
    assert!(meshes.get("ueb2101").is_none());
    assert!(Meshes::default().get("uel0001").is_none());
    assert_eq!(Meshes::default().model_set(), marker_models());
}

const GOOD_BLUEPRINT: &[u8] =
    b"UnitBlueprint {\n  Display = {\n    UniformScale = 0.105,\n  },\n}\n";

fn blueprint_with(display: &str) -> Vec<u8> {
    format!("UnitBlueprint {{\n  {display}\n}}\n").into_bytes()
}

fn step_of(result: Result<UnitMesh, RunError>) -> Option<(String, KindStep, String)> {
    match result {
        Err(RunError::UnitKind { kind, step, reason }) => Some((kind, step, reason)),
        _ => None,
    }
}

/// Each step's failure is its own error value naming the kind and the step,
/// the success carries the blueprint's scale and the model, and the scale is
/// refused unless it is finite and positive.
#[test]
fn supercom_unit_kind_resolution_failures() {
    // Success first: the scale of the blueprint and the model of the mesh.
    let built = build_kind("uel0001", &quad_mesh(), GOOD_BLUEPRINT);
    assert_eq!(
        built,
        Ok(UnitMesh {
            kind: "uel0001".to_owned(),
            scale: 0.105,
            model: model_of_mesh(&quad_mesh()),
        })
    );
    assert_eq!(model_of_mesh(&quad_mesh()).positions.len(), 4);

    for (display, what) in [
        ("", "no Display"),
        ("Display = { },", "no UniformScale"),
        ("Display = { UniformScale = 'big', },", "a string"),
        ("Display = { UniformScale = 0, },", "zero"),
        ("Display = { UniformScale = -0.5, },", "negative"),
        ("Display = { UniformScale = 1e39, },", "past f32"),
    ] {
        let failed = step_of(build_kind(
            "uel0001",
            &quad_mesh(),
            &blueprint_with(display),
        ));
        assert!(
            matches!(&failed, Some((kind, KindStep::Scale, _)) if kind == "uel0001"),
            "{what}: {failed:?}"
        );
    }

    let failed = step_of(build_kind(
        "url0001",
        &quad_mesh(),
        b"this is not a blueprint {{{",
    ));
    assert!(
        matches!(&failed, Some((kind, KindStep::BlueprintParse, _)) if kind == "url0001"),
        "{failed:?}"
    );

    // The scale is checked before the model.
    let mut short = quad_mesh();
    short.vertices[1].normal = [0.0; 3];
    let both = step_of(build_kind("uel0001", &short, &blueprint_with("")));
    assert!(matches!(&both, Some((_, KindStep::Scale, _))), "{both:?}");
    let refused = build_kind("uel0001", &short, GOOD_BLUEPRINT);
    let failed = step_of(refused.clone());
    assert!(
        matches!(&failed, Some((kind, KindStep::ModelCheck, reason))
            if kind == "uel0001" && reason.contains("vertex 1")),
        "{refused:?}"
    );
    let text = refused.err().map(|error| error.to_string());
    assert!(
        text.as_deref()
            .is_some_and(|t| t.contains("uel0001") && t.contains("model check")),
        "{text:?}"
    );

    // The reads and the mesh's parse, through a reader of fixed answers.
    let reader = |mesh: Option<&'static [u8]>, blueprint: Option<&'static [u8]>| {
        move |path: &str| -> Result<Vec<u8>, String> {
            let bytes = if path.ends_with("_lod0.scm") {
                mesh
            } else {
                blueprint
            };
            bytes
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("{path} is absent"))
        }
    };
    let missing = step_of(resolve_kind("ueb1103", &reader(None, Some(GOOD_BLUEPRINT))));
    assert!(
        matches!(&missing, Some((kind, KindStep::MeshRead, reason))
            if kind == "ueb1103" && reason.contains("units/ueb1103/ueb1103_lod0.scm")),
        "{missing:?}"
    );
    let missing = step_of(resolve_kind("ueb1103", &reader(Some(b"MODL"), None)));
    assert!(
        matches!(&missing, Some((kind, KindStep::BlueprintRead, reason))
            if kind == "ueb1103" && reason.contains("units/ueb1103/ueb1103_unit.bp")),
        "{missing:?}"
    );
    let junk = step_of(resolve_kind(
        "ueb1103",
        &reader(Some(b"not a mesh"), Some(GOOD_BLUEPRINT)),
    ));
    assert!(
        matches!(&junk, Some((kind, KindStep::MeshParse, _)) if kind == "ueb1103"),
        "{junk:?}"
    );
}

/// A reader of fixed answers: the mesh's bytes for a mesh path and the
/// blueprint's for any other, or an error naming the path when absent.
fn fixed_reader(
    mesh: Option<&'static [u8]>,
    blueprint: Option<&'static [u8]>,
) -> impl Fn(&str) -> Result<Vec<u8>, String> {
    move |path: &str| {
        let bytes = if path.ends_with("_lod0.scm") {
            mesh
        } else {
            blueprint
        };
        bytes
            .map(<[u8]>::to_vec)
            .ok_or_else(|| format!("{path} is absent"))
    }
}

/// Two defects at once give the earlier step's failure, in the order of
/// REN-084.
#[test]
fn supercom_unit_kind_resolution_reports_the_earlier_of_two_failures() {
    let both_absent = step_of(resolve_kind("ueb1103", &fixed_reader(None, None)));
    assert!(
        matches!(&both_absent, Some((_, KindStep::MeshRead, _))),
        "both files absent: {both_absent:?}"
    );
    let unparsed_and_absent = step_of(resolve_kind(
        "ueb1103",
        &fixed_reader(Some(b"not a mesh"), None),
    ));
    assert!(
        matches!(&unparsed_and_absent, Some((_, KindStep::BlueprintRead, _))),
        "a mesh that does not parse, a blueprint that is absent: {unparsed_and_absent:?}"
    );
    let neither_parses = step_of(resolve_kind(
        "ueb1103",
        &fixed_reader(Some(b"not a mesh"), Some(b"not a blueprint {{{")),
    ));
    assert!(
        matches!(&neither_parses, Some((_, KindStep::MeshParse, _))),
        "a mesh and a blueprint that both do not parse: {neither_parses:?}"
    );
}

/// The string literals of Rust source, comments left out: the text between
/// double quotes, with a backslash taking the next character.
fn string_literals(source: &str) -> Vec<String> {
    let (mut found, mut current) = (Vec::new(), String::new());
    let (mut in_string, mut in_comment) = (false, false);
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if in_comment {
            in_comment = c != '\n';
        } else if in_string {
            match c {
                '\\' => {
                    let _ = chars.next();
                }
                '"' => {
                    in_string = false;
                    found.push(std::mem::take(&mut current));
                }
                _ => current.push(c),
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            in_comment = true;
        } else if c == '"' {
            in_string = true;
        } else if c == '\'' {
            // A character literal: '"' must not open a string.
            if chars.peek() == Some(&'\\') {
                let _ = chars.next();
            }
            let _ = chars.next();
            if chars.peek() == Some(&'\'') {
                let _ = chars.next();
            }
        }
    }
    found
}

/// The `.rs` files under `dir`, not named for tests, found recursively.
fn source_files(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir);
    assert!(
        entries.is_ok(),
        "{dir:?} lists: {:?}",
        entries.as_ref().err()
    );
    let Ok(entries) = entries else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            source_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && name != "tests.rs"
            && name != "testing.rs"
        {
            into.push(path);
        }
    }
}

/// REN-082: the binary states no faction's name: no string literal of a
/// non-test source file holds one, and the walk read enough files that an
/// empty one cannot pass.
#[test]
fn supercom_binary_names_no_faction() {
    let mut files = Vec::new();
    source_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() >= 10, "read only {} files", files.len());
    for file in &files {
        let text = std::fs::read_to_string(file);
        assert!(text.is_ok(), "{file:?} reads: {:?}", text.as_ref().err());
        let text = text.unwrap_or_default();
        for literal in string_literals(&text) {
            for faction in ["UEF", "Aeon", "Cybran", "Seraphim"] {
                assert!(
                    !literal.contains(faction),
                    "{file:?} holds the literal {literal:?}"
                );
            }
        }
    }
    // The reader finds a literal past a comment, an escape, and a quote
    // character, so the walk above is not blind.
    let found =
        string_literals("// \"UEF\"\nlet a = '\"'; let b = \"x\\\"y\"; let c = \"Cybran\";");
    assert_eq!(found, ["xy".to_owned(), "Cybran".to_owned()]);
}

/// The text of a kind's failure holds the kind and the step's name as the
/// clause lists them, one for each of the six steps, and the text of a
/// mismatch holds its tick.
#[test]
fn supercom_unit_kind_failure_texts_name_the_step() {
    let text = |result: Result<UnitMesh, RunError>| result.err().map(|error| error.to_string());
    let mut short = quad_mesh();
    short.vertices[1].normal = [0.0; 3];

    let cases = [
        (
            "mesh read",
            text(resolve_kind(
                "uel0001",
                &fixed_reader(None, Some(GOOD_BLUEPRINT)),
            )),
        ),
        (
            "blueprint read",
            text(resolve_kind("uel0001", &fixed_reader(Some(b"MODL"), None))),
        ),
        (
            "mesh parse",
            text(resolve_kind(
                "uel0001",
                &fixed_reader(Some(b"not a mesh"), Some(GOOD_BLUEPRINT)),
            )),
        ),
        (
            "blueprint parse",
            text(build_kind("uel0001", &quad_mesh(), b"not a blueprint {{{")),
        ),
        (
            "scale",
            text(build_kind("uel0001", &quad_mesh(), &blueprint_with(""))),
        ),
        (
            "model check",
            text(build_kind("uel0001", &short, GOOD_BLUEPRINT)),
        ),
    ];
    for (name, text) in cases {
        let prefix = format!("unit kind uel0001: {name}: ");
        assert!(
            text.as_deref().is_some_and(|t| t.starts_with(&prefix)),
            "{name}: {text:?}"
        );
    }

    assert_eq!(
        RunError::Mismatch { tick: 99 }.to_string(),
        "replay: the digest at tick 99 differs from its checkpoint"
    );
}

/// Against an install that holds nothing the archives do not mount, and the
/// error says which step.
#[test]
fn supercom_unit_kinds_of_an_empty_install_are_an_error_value() {
    let nowhere = Path::new("/nonexistent/supercom-no-install");
    let failed = resolve_kinds(nowhere, &["uel0001".to_owned()]);
    assert!(
        matches!(&failed, Err(RunError::UnitArchives { .. })),
        "{failed:?}"
    );
    let text = failed
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(text.starts_with("unit meshes:"), "{text}");
}

fn mount(fa: &Path) -> Option<Meshes> {
    let kinds: Vec<String> = FIVE.iter().map(|kind| (*kind).to_owned()).collect();
    let meshes = resolve_kinds(fa, &kinds);
    assert!(
        meshes.is_ok(),
        "the five kinds resolve: {:?}",
        meshes.as_ref().err()
    );
    meshes.ok()
}

fn raw_mesh(fa: &Path, kind: &str) -> Option<Mesh> {
    let vfs = supercom_scd::Vfs::from_install(fa);
    assert!(vfs.is_ok(), "the install mounts: {:?}", vfs.as_ref().err());
    let bytes = vfs.ok()?.read(&mesh_path(kind));
    assert!(
        bytes.is_ok(),
        "{kind}'s mesh reads: {:?}",
        bytes.as_ref().err()
    );
    let mesh = Mesh::parse(&bytes.ok()?);
    assert!(
        mesh.is_ok(),
        "{kind}'s mesh parses: {:?}",
        mesh.as_ref().err()
    );
    mesh.ok()
}

/// The five kinds of the committed log: each lod0 mesh's vertex and index
/// counts and each blueprint's uniform scale, through the loaders.
#[test]
fn conformance_unit_meshes_sizes_and_scales() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let Some(meshes) = mount(&fa) else { return };
    for (kind, vertices, indices, scale) in [
        ("ueb1103", 1660, 2952, 0.06_f32),
        ("ueb2101", 1476, 2370, 0.155),
        ("uel0001", 5807, 10_458, 0.105),
        ("urb1101", 606, 1128, 0.225),
        ("url0001", 8702, 17_202, 0.16),
    ] {
        let found = meshes.get(kind);
        assert!(found.is_some(), "{kind} is resolved");
        let Some((_, unit)) = found else { return };
        assert_eq!(unit.model.positions.len(), vertices, "{kind}'s vertices");
        assert_eq!(unit.model.normals.len(), vertices, "{kind}'s normals");
        assert_eq!(unit.model.indices.len(), indices, "{kind}'s indices");
        assert!(
            unit.model
                .indices
                .iter()
                .all(|&i| usize::try_from(i).is_ok_and(|i| i < vertices)),
            "{kind}: no index at or past its vertex table"
        );
        assert!(
            (unit.scale - scale).abs() < 1e-6,
            "{kind}'s scale {}",
            unit.scale
        );
        let raw = raw_mesh(&fa, kind);
        assert_eq!(
            raw.map(|m| m.vertices.len()),
            Some(vertices),
            "{kind} through the loader"
        );
    }
    // The model set: four markers, then the five in sorted order.
    assert_eq!(meshes.model_set().len(), 9);
    for (place, kind) in FIVE.iter().enumerate() {
        assert_eq!(
            meshes.get(kind).map(|(index, _)| index),
            Some(4 + place),
            "{kind}"
        );
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn wide(v: [f32; 3]) -> [f64; 3] {
    v.map(f64::from)
}

/// The five kinds' frame, from the models the draw uses: the lowest stored y
/// (about 0 for four, -10.284 for ueb1103), counter-clockwise winding from
/// outside against the surface normals over the non-degenerate triangles, and
/// unit normals.
#[test]
fn conformance_unit_meshes_frame() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let Some(meshes) = mount(&fa) else { return };
    // (kind, lowest y, within, degenerate triangles)
    for (kind, lowest, within, degenerate) in [
        ("ueb1103", -10.284, 5e-4, 0_u32),
        ("ueb2101", 0.0, 0.05, 0),
        ("uel0001", 0.0, 0.05, 8),
        ("urb1101", 0.0, 0.05, 0),
        ("url0001", 0.0, 0.05, 0),
    ] {
        let found = meshes.get(kind);
        assert!(found.is_some(), "{kind} is resolved");
        let Some((_, unit)) = found else { return };
        let model = &unit.model;
        assert!(!model.positions.is_empty(), "{kind} has vertices");
        let lowest_y = model
            .positions
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        assert!(
            f64::from(lowest_y - lowest).abs() <= within,
            "{kind}: lowest y {lowest_y}"
        );
        let highest_y = model
            .positions
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            highest_y > 1.0,
            "{kind}: +y is up, the top is at {highest_y}"
        );

        let (mut live, mut agree, mut flat) = (0_u32, 0_u32, 0_u32);
        for corner in model.indices.chunks_exact(3) {
            let at = |k: usize| usize::try_from(corner[k]).unwrap_or(usize::MAX);
            let [a, b, c] = [at(0), at(1), at(2)].map(|i| wide(model.positions[i]));
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let face = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            if dot(face, face) == 0.0 {
                flat += 1;
                continue;
            }
            live += 1;
            let mut sum = [0.0; 3];
            for i in [at(0), at(1), at(2)] {
                let n = wide(model.normals[i]);
                sum = [sum[0] + n[0], sum[1] + n[1], sum[2] + n[2]];
            }
            if dot(face, sum) > 0.0 {
                agree += 1;
            }
        }
        assert_eq!(flat, degenerate, "{kind}: degenerate triangles");
        assert!(live > 0, "{kind}: has live triangles");
        let share = f64::from(agree) / f64::from(live.max(1));
        assert!(share >= 0.999_95, "{kind}: winding share {share}");

        let lengths: Vec<f64> = model
            .normals
            .iter()
            .map(|n| dot(wide(*n), wide(*n)).sqrt())
            .collect();
        assert!(
            lengths.iter().all(|l| (l - 1.0).abs() <= 1e-3),
            "{kind}: a normal is off unit length by more than 1e-3"
        );
        assert!(
            lengths.iter().all(|l| *l >= 1e-6),
            "{kind}: a normal is shorter than 1e-6"
        );
    }
}

/// A bone's place under the stored inverse transposed: with M the transpose
/// of the stored matrix acting on column vectors, M = [R t; 0 1], minus the
/// transpose of R times t.
fn bone_place(stored: &[[f32; 4]; 4]) -> [f64; 3] {
    let m = |r: usize, c: usize| f64::from(stored[c][r]);
    let t = [m(0, 3), m(1, 3), m(2, 3)];
    [0, 1, 2].map(|k| -(m(0, k) * t[0] + m(1, k) * t[1] + m(2, k) * t[2]))
}

/// The five kinds' bones: the Muzzle bones' mean z under the stored inverse
/// transposed, and how many bones with ten or more vertices, farther from the
/// origin than a tenth of the mesh's largest extent, have their vertices'
/// centroid nearer the bone than the origin.
#[test]
fn conformance_unit_meshes_bones() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let Some(meshes) = mount(&fa) else { return };
    // (kind, Muzzle bones, their mean z, far bones, of them centroid-near)
    for (kind, muzzles, mean_z, far, near) in [
        ("ueb1103", 0_u32, 0.0, 11_u32, 9_u32),
        ("ueb2101", 1, 3.156, 2, 2),
        ("uel0001", 5, 7.641, 19, 19),
        ("urb1101", 0, 0.0, 0, 0),
        ("url0001", 3, 4.024, 18, 18),
    ] {
        let found = meshes.get(kind);
        assert!(found.is_some(), "{kind} is resolved");
        let Some((_, unit)) = found else { return };
        let Some(mesh) = raw_mesh(&fa, kind) else {
            return;
        };
        let positions = &unit.model.positions;
        assert_eq!(
            positions.len(),
            mesh.vertices.len(),
            "{kind}: the model keeps every vertex"
        );

        let (mut muzzle_count, mut muzzle_z) = (0_u32, 0.0);
        for bone in mesh.bones.iter().filter(|b| b.name.contains("Muzzle")) {
            muzzle_count += 1;
            muzzle_z += bone_place(&bone.rest_pose_inverse)[2];
        }
        assert_eq!(muzzle_count, muzzles, "{kind}: Muzzle bones");
        if muzzle_count > 0 {
            let mean = muzzle_z / f64::from(muzzle_count);
            assert!((mean - mean_z).abs() < 5e-4, "{kind}: mean Muzzle z {mean}");
        }

        let mut sums = vec![([0.0_f64; 3], 0_u32); mesh.bones.len()];
        for (vertex, position) in mesh.vertices.iter().zip(positions) {
            if let Some((sum, count)) = sums.get_mut(usize::from(vertex.bone_indices[0])) {
                for (total, value) in sum.iter_mut().zip(wide(*position)) {
                    *total += value;
                }
                *count += 1;
            }
        }
        let extent = (0..3)
            .map(|axis| {
                let values = positions.iter().map(|p| p[axis]);
                f64::from(
                    values.clone().fold(f32::NEG_INFINITY, f32::max)
                        - values.fold(f32::INFINITY, f32::min),
                )
            })
            .fold(0.0, f64::max);
        let (mut far_bones, mut near_bones) = (0_u32, 0_u32);
        for (bone, (sum, count)) in mesh.bones.iter().zip(&sums) {
            if *count < 10 {
                continue;
            }
            let place = bone_place(&bone.rest_pose_inverse);
            if dot(place, place).sqrt() <= 0.1 * extent {
                continue;
            }
            far_bones += 1;
            let centroid = sum.map(|s| s / f64::from(*count));
            let to_bone = [0, 1, 2].map(|k| centroid[k] - place[k]);
            if dot(to_bone, to_bone) < dot(centroid, centroid) {
                near_bones += 1;
            }
        }
        assert_eq!(
            (far_bones, near_bones),
            (far, near),
            "{kind}: rest-pose bones"
        );
    }
}
