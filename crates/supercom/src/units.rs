//! The unit kinds a replay names and their meshes: which kinds, how a mesh
//! becomes a renderer model, and the resolving of each kind to a mesh and a
//! scale before the window. Pure but for [`resolve_kinds`], which reads the
//! install.

use crate::error::{KindStep, RunError};
use crate::markers::{MARKER_COUNT, marker_models};
use std::collections::BTreeSet;
use std::path::Path;
use supercom_render::Model;
use supercom_scd::Vfs;
use supercom_scm::Mesh;
use supercom_sim::{Replay, State};

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

/// The resolved kinds of a replay, in the sorted order of their ids.
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
        let _ = (&self.meshes, kind, MARKER_COUNT);
        None
    }

    /// The set the renderer uploads once: the four markers, then one model
    /// per resolved kind in the order of [`Meshes::kinds`].
    #[must_use]
    pub fn model_set(&self) -> Vec<Model> {
        let _ = &self.meshes;
        marker_models()
    }

    /// The distinct kinds `state` holds that were not resolved, by the
    /// blueprint name each entity carries.
    #[must_use]
    pub fn unnamed(&self, state: &State) -> BTreeSet<String> {
        let _ = (&self.meshes, state);
        BTreeSet::new()
    }
}

/// The model of `mesh`: the stored positions, the stored normals, and the
/// indices widened to 32 bits.
#[must_use]
pub fn model_of_mesh(mesh: &Mesh) -> Model {
    let _ = mesh;
    Model {
        positions: Vec::new(),
        normals: Vec::new(),
        indices: Vec::new(),
    }
}

/// The lower-case, sorted, distinct ids of the kinds `replay` names: each
/// player's commander and every build order's blueprint.
#[must_use]
pub fn kinds_named(replay: &Replay) -> Vec<String> {
    let _ = replay;
    Vec::new()
}

/// Resolves one kind from its two files' bytes and its parsed mesh: the
/// blueprint parse, the scale, and the model with the renderer's check.
///
/// # Errors
/// [`RunError::UnitKind`] naming the kind and the first step that failed:
/// [`KindStep::BlueprintParse`], [`KindStep::Scale`], then
/// [`KindStep::ModelCheck`].
pub fn build_kind(kind: &str, mesh: &Mesh, blueprint: &[u8]) -> Result<UnitMesh, RunError> {
    let _ = (mesh, blueprint);
    Err(RunError::UnitKind {
        kind: kind.to_owned(),
        step: KindStep::BlueprintParse,
        reason: model_of_mesh(mesh).positions.len().to_string(),
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
