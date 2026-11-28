use super::*;
use crate::mapfile::MapFileError;
use std::fs;
use std::path::PathBuf;

/// A directory under the system's temporary directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("supercom-plan-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the scratch directory is created");
        Self(dir)
    }

    fn file(&self, folder: &str, name: &str, bytes: &[u8]) {
        let dir = self.0.join("maps").join(folder);
        fs::create_dir_all(&dir).expect("the map folder is created");
        fs::write(dir.join(name), bytes).expect("the file is written");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Each step of the plan that needs no window gives its own failure.
#[test]
fn supercom_plan_run_failures() {
    assert_eq!(plan_run(None, "m").err(), Some(RunError::InstallUnresolved));

    let missing = std::env::temp_dir().join("supercom-plan-no-such-install");
    assert_eq!(
        plan_run(Some(&missing), "m").err(),
        Some(RunError::InstallMissing { dir: missing })
    );

    let root = Scratch::new("failures");
    let install_file = root.0.join("install-is-a-file");
    fs::write(&install_file, b"not a directory").expect("the file is written");
    assert_eq!(
        plan_run(Some(&install_file), "m").err(),
        Some(RunError::InstallMissing { dir: install_file }),
        "an install that is a regular file is a missing install"
    );
    assert!(
        matches!(
            plan_run(Some(&root.0), "absent").err(),
            Some(RunError::MapFolder { folder, .. }) if folder == "absent"
        ),
        "a folder that cannot be listed names itself"
    );

    root.file("none", "readme.lua", b"-- no map here");
    assert_eq!(
        plan_run(Some(&root.0), "none").err(),
        Some(RunError::MapFile(MapFileError::NoMapFile {
            folder: "none".to_owned()
        }))
    );

    root.file("two", "a.scmap", b"x");
    root.file("two", "B.SCMAP", b"y");
    assert!(
        matches!(
            plan_run(Some(&root.0), "two").err(),
            Some(RunError::MapFile(MapFileError::AmbiguousMapFile { .. }))
        ),
        "two map files are ambiguous"
    );

    root.file("junk", "junk.scmap", b"this is not a map");
    assert!(
        matches!(
            plan_run(Some(&root.0), "junk").err(),
            Some(RunError::ParseMap { path, .. }) if path.ends_with("junk.scmap")
        ),
        "a file that does not parse names itself"
    );
}

/// Against the install: the map of `SCMP_012` is found and parsed.
#[test]
fn conformance_plan_loads_an_installed_map() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let map = plan_run(Some(&fa), "SCMP_012");
    assert!(map.is_ok(), "SCMP_012 loads: {:?}", map.as_ref().err());
    let dims = map.map(|(map, _)| (map.heightmap.width, map.heightmap.height));
    assert_eq!(dims.ok(), Some((256, 256)));
}
