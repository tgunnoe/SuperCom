//! Synthetic suite of `supercom-bp-import`: no archive and no map is opened;
//! the install paths named here do not exist. Against the skeleton every
//! test fails on a function returning `Err(Error::Unimplemented)`.

use std::path::Path;

use supercom_bp_import::{Error, cache_key, import, load};

/// An install directory that does not exist.
const ABSENT: &str = "/nonexistent/supercom-install";

/// Pins the error path of `import`: an install that is not there fails with
/// `Error::Archive` naming the archive path, not with a panic or an empty
/// catalog.
#[test]
fn import_of_an_absent_install_is_an_archive_error() {
    let result = import(Path::new(ABSENT));
    assert!(
        matches!(&result, Err(Error::Archive { path, .. }) if path.contains("units.scd")),
        "expected Archive naming units.scd, got {:?}",
        result.as_ref().err()
    );
}

/// Pins the error path of `cache_key`: the key of an absent install is an
/// `Archive` error, so a key never stands for nothing.
#[test]
fn cache_key_of_an_absent_install_is_an_archive_error() {
    let result = cache_key(Path::new(ABSENT));
    assert!(
        matches!(result, Err(Error::Archive { .. })),
        "expected Archive, got {result:?}"
    );
}

/// Pins the miss path of `load`: with nothing cached and no install, `load`
/// falls through to the import and returns its `Archive` error, and writes
/// no cache file.
#[test]
fn load_without_a_cache_or_install_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let result = load(Path::new(ABSENT), dir.path());
    assert!(
        matches!(result, Err(Error::Archive { .. })),
        "expected Archive, got {result:?}"
    );
    let written = std::fs::read_dir(dir.path()).map(Iterator::count);
    assert_eq!(written.ok(), Some(0), "a failed import must leave no cache");
}

/// Pins the unwritable-cache path: a cache directory that is a regular file
/// is a `Cache` error (or an earlier `Archive` one for the absent install),
/// never `Unimplemented`.
#[test]
fn load_into_a_file_instead_of_a_directory_is_not_unimplemented() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"x").expect("scratch file");
    let result = load(Path::new(ABSENT), &file);
    assert!(
        matches!(result, Err(Error::Cache { .. } | Error::Archive { .. })),
        "expected Cache or Archive, got {result:?}"
    );
}

/// Why the cache's encoding must be correctly rounded: `serde_json` without
/// `float_roundtrip` parses some literals one ulp off.
#[cfg(test)]
mod float_bits {
    /// Encodes and decodes `values` through the cache's own serializer and
    /// deserializer (`serde_json::to_vec` / `from_slice`).
    fn round_trip(values: &[f64]) -> Result<Vec<f64>, serde_json::Error> {
        let bytes = serde_json::to_vec(values)?;
        serde_json::from_slice(&bytes)
    }

    fn assert_bits_equal(values: &[f64]) {
        let back = round_trip(values);
        assert!(back.is_ok(), "round trip failed: {:?}", back.as_ref().err());
        let back = back.unwrap_or_default();
        assert_eq!(back.len(), values.len());
        for (a, b) in values.iter().zip(&back) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "{a:e} came back as {b:e}: bits {:#018x} vs {:#018x}",
                a.to_bits(),
                b.to_bits()
            );
        }
    }

    /// Pins bit-exact bits for a fixed literal set: edge values and the
    /// literal found to misparse without `float_roundtrip`.
    #[test]
    fn round_trip_of_literals_keeps_bits() {
        let mut values = vec![
            0.0,
            -0.0,
            0.1,
            1.0 / 3.0,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            1e23,
            5e-324,
            2.2250738585072011e-308,
        ];
        values.extend(MISPARSED_BITS.map(f64::from_bits));
        assert_bits_equal(&values);
    }

    /// Bit patterns that misparse without the feature, found by sweep:
    /// `-1.1193133179981887e-17` (0xbc69cf4276846d19) parses as
    /// 0xbc69cf4276846d1a.
    const MISPARSED_BITS: [u64; 1] = [0xbc69_cf42_7684_6d19];

    /// Pins bit-exact bits over 100,000 finite patterns from a seeded
    /// linear congruential generator.
    #[test]
    fn round_trip_sweep_of_bit_patterns_keeps_bits() {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut values = Vec::with_capacity(100_000);
        while values.len() < 100_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let v = f64::from_bits(state);
            if v.is_finite() {
                values.push(v);
            }
        }
        assert_bits_equal(&values);
    }
}
