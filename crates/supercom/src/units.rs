//! The unit kinds a run resolves and their meshes: which kinds, how a mesh
//! becomes a renderer model, and the resolving of each kind to a mesh and a
//! scale before the window. Pure but for [`resolve_kinds`], which reads the
//! install.

use crate::error::{KindStep, RunError};
use crate::markers::{MARKER_COUNT, marker_models};
use std::collections::BTreeSet;
use std::path::Path;
use supercom_bp::Blueprint;
use supercom_render::{Model, check_model};
use supercom_scd::Vfs;
use supercom_scm::Mesh;
use supercom_sim::{Order, Replay, State};

/// One resolved kind: its lower-case id, the blueprint's uniform scale, and
/// the model built from its lod0 mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitMesh {
    /// The kind's lower-case id.
    pub kind: String,
    /// The blueprint's `Display.UniformScale`, finite and positive.
    pub scale: f32,
    /// The model built from the mesh, in the mesh's own coordinates.
    pub model: Model,
}

/// The resolved kinds of a run, in the sorted order of their ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meshes {
    meshes: Vec<UnitMesh>,
}

impl Meshes {
    /// The resolved kinds, `meshes` taken in the order given, which the
    /// model set follows.
    #[must_use]
    pub fn new(meshes: Vec<UnitMesh>) -> Self {
        Self { meshes }
    }

    /// The model-set index and the mesh of `kind`, if it was resolved.
    #[must_use]
    pub fn get(&self, kind: &str) -> Option<(usize, &UnitMesh)> {
        let place = self.meshes.iter().position(|mesh| mesh.kind == kind)?;
        self.meshes
            .get(place)
            .map(|mesh| (MARKER_COUNT + place, mesh))
    }

    /// The set the renderer uploads once: the four markers, then one model
    /// per resolved kind in the order this set holds them.
    #[must_use]
    pub fn model_set(&self) -> Vec<Model> {
        let mut set = marker_models();
        set.extend(self.meshes.iter().map(|mesh| mesh.model.clone()));
        set
    }

    /// The distinct kinds `state` holds that were not resolved, by the
    /// blueprint name each entity carries.
    #[must_use]
    pub fn unnamed(&self, state: &State) -> BTreeSet<String> {
        state
            .entities
            .values()
            .filter(|entity| self.get(&entity.blueprint).is_none())
            .map(|entity| entity.blueprint.clone())
            .collect()
    }
}

/// The model of `mesh`: the stored positions, the stored normals, and the
/// indices widened to 32 bits. Nothing else of the mesh is read: not its
/// bones, their inverse matrices, the tangents, the binormals, or the
/// texture coordinates.
#[must_use]
pub fn model_of_mesh(mesh: &Mesh) -> Model {
    Model {
        positions: mesh.vertices.iter().map(|v| v.position).collect(),
        normals: mesh.vertices.iter().map(|v| v.normal).collect(),
        indices: mesh.indices.iter().map(|&i| u32::from(i)).collect(),
    }
}

/// The lower-case, sorted, distinct ids of the kinds a run resolves: the
/// blueprint of every entity of the start state and of every build order,
/// mobile or factory.
#[must_use]
pub fn run_kinds(state: &State, replay: &Replay) -> Vec<String> {
    let placed = state
        .entities
        .values()
        .map(|entity| entity.blueprint.to_ascii_lowercase());
    let ordered = replay
        .commands
        .iter()
        .filter_map(|command| match &command.order {
            Order::BuildMobile { blueprint, .. } | Order::BuildFactory { blueprint } => {
                Some(blueprint.to_ascii_lowercase())
            }
            _ => None,
        });
    placed
        .chain(ordered)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The blueprint's `Display.UniformScale` as an `f32`.
///
/// # Errors
/// A sentence when the blueprint holds no such number, or when it is not
/// finite and positive as an `f32`.
fn scale_of(blueprint: &Blueprint) -> Result<f32, String> {
    let value = blueprint
        .body
        .get("Display")
        .and_then(|display| display.get("UniformScale"))
        .ok_or("the blueprint holds no Display.UniformScale")?
        .as_f64()
        .ok_or("Display.UniformScale is not a number")?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "std has no f64 to f32 conversion; a value past f32 is refused as not finite below"
    )]
    let scale = value as f32;
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(format!(
            "Display.UniformScale {value} is not a finite positive number"
        ))
    }
}

/// Resolves one kind from its two files' bytes and its parsed mesh: the
/// blueprint parse, the scale, and the model with the renderer's check.
///
/// # Errors
/// [`RunError::UnitKind`] naming the kind and the first step that failed:
/// [`KindStep::BlueprintParse`], [`KindStep::Scale`], then
/// [`KindStep::ModelCheck`].
pub fn build_kind(kind: &str, mesh: &Mesh, blueprint: &[u8]) -> Result<UnitMesh, RunError> {
    let failed = |step, reason: String| RunError::UnitKind {
        kind: kind.to_owned(),
        step,
        reason,
    };
    let parsed = supercom_bp::parse_bytes(blueprint)
        .map_err(|error| failed(KindStep::BlueprintParse, error.to_string()))?;
    let scale = scale_of(&parsed).map_err(|reason| failed(KindStep::Scale, reason))?;
    let model = model_of_mesh(mesh);
    check_model(0, &model).map_err(|error| failed(KindStep::ModelCheck, error.to_string()))?;
    Ok(UnitMesh {
        kind: kind.to_owned(),
        scale,
        model,
    })
}

/// The path of a kind's lod0 mesh, by its lower-case id.
fn mesh_path(kind: &str) -> String {
    format!("units/{kind}/{kind}_lod0.scm")
}

/// The path of a kind's blueprint, by its lower-case id.
fn blueprint_path(kind: &str) -> String {
    format!("units/{kind}/{kind}_unit.bp")
}

/// Resolves one kind, reading its two files through `read`.
///
/// # Errors
/// [`RunError::UnitKind`] naming the kind and the first step that failed:
/// [`KindStep::MeshRead`], [`KindStep::BlueprintRead`],
/// [`KindStep::MeshParse`], then the steps of [`build_kind`].
pub fn resolve_kind(
    kind: &str,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<UnitMesh, RunError> {
    let failed = |step, reason: String| RunError::UnitKind {
        kind: kind.to_owned(),
        step,
        reason,
    };
    let mesh_bytes = read(&mesh_path(kind)).map_err(|reason| failed(KindStep::MeshRead, reason))?;
    let blueprint =
        read(&blueprint_path(kind)).map_err(|reason| failed(KindStep::BlueprintRead, reason))?;
    let mesh =
        Mesh::parse(&mesh_bytes).map_err(|error| failed(KindStep::MeshParse, error.to_string()))?;
    build_kind(kind, &mesh, &blueprint)
}

/// Resolves every kind of `kinds` from the install at `install`.
///
/// # Errors
/// [`RunError::UnitArchives`] when the archives do not mount, and
/// [`RunError::UnitKind`] for the first kind that does not resolve.
pub fn resolve_kinds(install: &Path, kinds: &[String]) -> Result<Meshes, RunError> {
    let vfs = Vfs::from_install(install).map_err(|error| RunError::UnitArchives {
        reason: error.to_string(),
    })?;
    let read = |path: &str| vfs.read(path).map_err(|error| error.to_string());
    kinds
        .iter()
        .map(|kind| resolve_kind(kind, &read))
        .collect::<Result<Vec<_>, _>>()
        .map(Meshes::new)
}

#[cfg(test)]
mod tests;
