use std::path::PathBuf;
use std::ptr::null_mut;

use anyhow::Result;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows::core::{HSTRING, w};

pub(crate) fn chewing_dll_version() -> String {
    let Ok(dll_path) = program_dir().map(|path| path.join("chewing_tip.dll")) else {
        return String::from("0.0.0.0");
    };

    let h_dll_path: HSTRING = dll_path.into_os_string().into();

    unsafe {
        let size = GetFileVersionInfoSizeW(&h_dll_path, None);
        if size == 0 {
            return String::from("0.0.0.0");
        }
        let mut lpdata = vec![0u8; size as usize];
        let mut file_info: *mut VS_FIXEDFILEINFO = null_mut();
        let pfile_info: *mut *mut VS_FIXEDFILEINFO = &mut file_info;
        let mut pulen = 0u32;
        if GetFileVersionInfoW(&h_dll_path, None, size, lpdata.as_mut_ptr().cast()).is_ok()
            && VerQueryValueW(
                lpdata.as_ptr().cast(),
                w!("\\"),
                pfile_info.cast(),
                &mut pulen,
            )
            .as_bool()
        {
            return format!(
                "{}.{}.{}.{}",
                hi_word((*file_info).dwProductVersionMS),
                lo_word((*file_info).dwProductVersionMS),
                hi_word((*file_info).dwProductVersionLS),
                lo_word((*file_info).dwProductVersionLS)
            );
        }
    }
    "0.0.0.0".to_string()
}

pub(crate) fn chewing_dll_channel() -> String {
    let (_, _, _, build) = parse_version(&chewing_dll_version());
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

fn program_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("ProgramW6432")
            .or_else(|_| std::env::var("ProgramFiles"))
            .or_else(|_| std::env::var("FrogramFiles(x86)"))?,
    )
    .join("ChewingTextService"))
}

pub const fn hi_word(v: u32) -> u16 {
    (v >> 16 & 0xffff) as _
}

pub const fn lo_word(v: u32) -> u16 {
    (v & 0xffff) as _
}

#[cfg(test)]
mod tests {
    use crate::version::{parse_version, version_gt};

    #[test]
    fn parse_version_test() {
        let ver = "25.10.0.477";
        let ver_tuple = parse_version(ver);
        assert_eq!((25, 10, 0, 477), ver_tuple);
    }

    #[test]
    fn compare_test() {
        let v1 = "25.10.0.476";
        let v2 = "25.10.0.477";
        assert!(!version_gt(v1, v2));
        assert!(version_gt(v2, v1));
    }
}
