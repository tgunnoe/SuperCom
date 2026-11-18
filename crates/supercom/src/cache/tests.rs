use super::*;

fn vars(over: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> CacheVars {
    CacheVars {
        override_dir: over.map(OsString::from),
        xdg_cache_home: xdg.map(OsString::from),
        home: home.map(OsString::from),
    }
}

/// REN-059: each of the three variables is read under its own name: a lookup
/// that answers one exact name sets that field alone, and a lookup that
/// answers no name sets none.
#[test]
fn supercom_cache_vars_are_read_under_their_names() {
    let only = |name: &'static str| {
        let lookup = move |asked: &str| (asked == name).then(|| OsString::from("/value"));
        cache_vars_from(&lookup)
    };
    let none = cache_vars_from(&|_: &str| None);
    assert_eq!(
        (none.override_dir, none.xdg_cache_home, none.home),
        (None, None, None)
    );

    let over = only("SUPERCOM_CACHE_DIR");
    assert_eq!(
        (over.override_dir, over.xdg_cache_home, over.home),
        (Some(OsString::from("/value")), None, None)
    );
    let xdg = only("XDG_CACHE_HOME");
    assert_eq!(
        (xdg.override_dir, xdg.xdg_cache_home, xdg.home),
        (None, Some(OsString::from("/value")), None)
    );
    let home = only("HOME");
    assert_eq!(
        (home.override_dir, home.xdg_cache_home, home.home),
        (None, None, Some(OsString::from("/value")))
    );
}

/// Each rule, in order, with an empty value falling through, and the
/// refusals.
#[test]
fn supercom_cache_dir_cases() {
    let all = vars(Some("/c/over"), Some("/x"), Some("/h"));
    assert_eq!(cache_dir(&all), Ok(PathBuf::from("/c/over")));
    let no_override = vars(None, Some("/x"), Some("/h"));
    assert_eq!(cache_dir(&no_override), Ok(PathBuf::from("/x/supercom")));
    let empty_override = vars(Some(""), Some("/x"), Some("/h"));
    assert_eq!(cache_dir(&empty_override), Ok(PathBuf::from("/x/supercom")));
    let empty_xdg = vars(None, Some(""), Some("/h"));
    assert_eq!(
        cache_dir(&empty_xdg),
        Ok(PathBuf::from("/h/.cache/supercom"))
    );
    let home_only = vars(None, None, Some("/h"));
    assert_eq!(
        cache_dir(&home_only),
        Ok(PathBuf::from("/h/.cache/supercom"))
    );

    let relative_override = vars(Some("rel"), Some("/x"), Some("/h"));
    assert_eq!(
        cache_dir(&relative_override),
        Err(CacheDirError::Relative {
            path: PathBuf::from("rel")
        }),
        "a relative override is refused, not skipped"
    );
    let relative_xdg = vars(None, Some("rel"), Some("/h"));
    assert_eq!(
        cache_dir(&relative_xdg),
        Err(CacheDirError::Relative {
            path: PathBuf::from("rel/supercom")
        })
    );
    let relative_home = vars(None, None, Some("h"));
    assert_eq!(
        cache_dir(&relative_home),
        Err(CacheDirError::Relative {
            path: PathBuf::from("h/.cache/supercom")
        })
    );

    assert_eq!(
        cache_dir(&vars(None, None, None)),
        Err(CacheDirError::Unresolved)
    );
    assert_eq!(
        cache_dir(&vars(Some(""), Some(""), Some(""))),
        Err(CacheDirError::Unresolved)
    );
}
