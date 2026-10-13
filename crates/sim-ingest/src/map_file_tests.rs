//! The map-file matcher against temporary folders; no install.

use std::fs;

use super::{Error, read_map_file};

fn folder_with(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary folder");
    for name in names {
        fs::write(dir.path().join(name), name.as_bytes()).expect("a file");
    }
    dir
}

#[test]
fn guard_a_folder_without_the_suffix_is_missing_it() {
    let dir = folder_with(&["map_save.lua", "readme.txt"]);
    assert!(matches!(
        read_map_file(dir.path(), "m", ".scmap"),
        Err(Error::MissingMapFile { folder, suffix }) if folder == "m" && suffix == ".scmap"
    ));
}

#[test]
fn guard_two_files_with_the_suffix_are_ambiguous() {
    let dir = folder_with(&["a.scmap", "b.SCMAP", "c_save.lua"]);
    assert!(matches!(
        read_map_file(dir.path(), "m", ".scmap"),
        Err(Error::AmbiguousMapFile { count: 2, .. })
    ));
}

#[test]
fn guard_the_suffix_matches_in_any_case() {
    let dir = folder_with(&["Prefix.SCMAP", "other_SAVE.LUA"]);
    assert_eq!(
        read_map_file(dir.path(), "m", ".scmap").expect("the file is found"),
        b"Prefix.SCMAP"
    );
    assert_eq!(
        read_map_file(dir.path(), "m", "_save.lua").expect("the file is found"),
        b"other_SAVE.LUA"
    );
}
