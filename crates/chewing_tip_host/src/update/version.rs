use std::fs;

use chewing_tip_core::shell::program_dir;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProductVersion {
    product_version: String,
    #[allow(unused)]
    build_date: String,
}

pub(crate) fn chewing_product_version() -> String {
    let default = String::from("0.0.0.0");
    let Ok(json_path) = program_dir().map(|path| path.join("version.json")) else {
        return default;
    };
    let Ok(json) = fs::read_to_string(json_path) else {
        return default;
    };
    let pv = serde_json::from_str::<ProductVersion>(&json);
    pv.map(|v| v.product_version).unwrap_or(default)
}

pub(crate) fn chewing_dll_channel() -> String {
    let (_, _, _, build) = parse_version(&chewing_product_version());
    if build == 0 {
        "stable".to_string()
    } else {
        "development".to_string()
    }
}

fn parse_version(ver: &str) -> (u64, u64, u64, u64) {
    let mut parts = ver.split('.');
    (
        parts
            .next()
            .map(|s| s.parse().unwrap_or_default())
            .unwrap_or_default(),
        parts
            .next()
            .map(|s| s.parse().unwrap_or_default())
            .unwrap_or_default(),
        parts
            .next()
            .map(|s| s.parse().unwrap_or_default())
            .unwrap_or_default(),
        parts
            .next()
            .map(|s| s.parse().unwrap_or_default())
            .unwrap_or_default(),
    )
}

pub(crate) fn version_gt(ver_a: &str, ver_b: &str) -> bool {
    let (o_major, o_minor, o_patch, o_build) = parse_version(ver_b);
    let (n_major, n_minor, n_patch, n_build) = parse_version(ver_a);

    if n_major > o_major {
        return true;
    }
    if n_major < o_major {
        return false;
    }
    if n_minor > o_minor {
        return true;
    }
    if n_minor < o_minor {
        return false;
    }
    if n_patch > o_patch {
        return true;
    }
    if n_patch < o_patch {
        return false;
    }
    if n_build > o_build {
        return true;
    }
    if n_build < o_build {
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{parse_version, version_gt};

    #[test]
    fn parse_version_test() {
        let ver = "25.10.0.477";
        let ver_tuple = parse_version(ver);
        assert_eq!((25, 10, 0, 477), ver_tuple);
    }

    #[test]
    fn compare_build_test() {
        let v1 = "25.10.0.476";
        let v2 = "25.10.0.477";
        assert!(!version_gt(v1, v2));
        assert!(version_gt(v2, v1));
    }

    #[test]
    fn compare_patch_test() {
        let v1 = "25.10.0.476";
        let v2 = "25.10.1.477";
        assert!(!version_gt(v1, v2));
        assert!(version_gt(v2, v1));
    }

    #[test]
    fn compare_minor_test() {
        let v1 = "25.10.0.476";
        let v2 = "25.11.0.477";
        assert!(!version_gt(v1, v2));
        assert!(version_gt(v2, v1));
    }

    #[test]
    fn compare_major_test() {
        let v1 = "25.10.0.476";
        let v2 = "26.01.0.477";
        assert!(!version_gt(v1, v2));
        assert!(version_gt(v2, v1));
    }
}
