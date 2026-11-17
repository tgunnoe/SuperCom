use super::*;

fn names(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn supercom_choose_map_file_cases() {
    assert_eq!(
        choose_map_file(
            "SCMP_012",
            &names(&[
                "SCMP_012.scmap",
                "SCMP_012_scenario.lua",
                "SCMP_012_save.lua"
            ])
        ),
        Ok(OsString::from("SCMP_012.scmap"))
    );
    assert_eq!(
        choose_map_file("m", &names(&["a_script.lua", "Map.SCMAP"])),
        Ok(OsString::from("Map.SCMAP")),
        "the suffix is matched ignoring case"
    );
    assert_eq!(
        choose_map_file("m", &names(&["x.scmap.bak", "x.scmap_old", "scmap"])),
        Err(MapFileError::NoMapFile {
            folder: "m".to_owned()
        }),
        "only a name that ends in .scmap counts"
    );
    assert_eq!(
        choose_map_file("empty", &[]),
        Err(MapFileError::NoMapFile {
            folder: "empty".to_owned()
        })
    );
    let two = choose_map_file("two", &names(&["b.scmap", "A.SCMAP", "c.lua"]));
    assert_eq!(
        two,
        Err(MapFileError::AmbiguousMapFile {
            folder: "two".to_owned(),
            names: vec!["A.SCMAP".to_owned(), "b.scmap".to_owned()],
        })
    );
    let text = two.err().map(|error| error.to_string());
    assert!(
        text.as_deref().is_some_and(|text| text.contains("two")),
        "the error names the folder: {text:?}"
    );
}
