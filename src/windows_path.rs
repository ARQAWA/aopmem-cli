//! Shared Windows path conversion for direct Win32 filesystem calls.

#[cfg(windows)]
use std::io;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
#[cfg(windows)]
use std::path::{Path, PathBuf};

#[cfg(any(windows, test))]
const VERBATIM_PREFIX: &[u16] = &[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16];
#[cfg(any(windows, test))]
const VERBATIM_UNC_PREFIX: &[u16] = &[
    b'\\' as u16,
    b'\\' as u16,
    b'?' as u16,
    b'\\' as u16,
    b'U' as u16,
    b'N' as u16,
    b'C' as u16,
    b'\\' as u16,
];

#[cfg(windows)]
pub(crate) fn verbatim_wide_path(path: &Path) -> io::Result<Vec<u16>> {
    validated_verbatim_wide(
        &path.as_os_str().encode_wide().collect::<Vec<_>>(),
        path.is_absolute(),
    )
}

#[cfg(windows)]
pub(crate) fn verbatim_path(path: &Path) -> io::Result<PathBuf> {
    let mut wide = verbatim_wide_path(path)?;
    if wide.last() == Some(&0) {
        wide.pop();
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

#[cfg(any(windows, test))]
fn validated_verbatim_wide(raw: &[u16], absolute: bool) -> std::io::Result<Vec<u16>> {
    if !absolute {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Windows API path must be absolute",
        ));
    }
    if raw.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Windows API path contains NUL",
        ));
    }
    let mut wide = if raw.starts_with(VERBATIM_PREFIX) {
        Vec::from(raw)
    } else if raw.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut value = Vec::with_capacity(VERBATIM_UNC_PREFIX.len() + raw.len() - 2 + 1);
        value.extend_from_slice(VERBATIM_UNC_PREFIX);
        value.extend_from_slice(&raw[2..]);
        value
    } else {
        let mut value = Vec::with_capacity(VERBATIM_PREFIX.len() + raw.len() + 1);
        value.extend_from_slice(VERBATIM_PREFIX);
        value.extend_from_slice(raw);
        value
    };
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    #[test]
    fn normal_unc_and_existing_verbatim_paths_have_one_prefix_and_nul() {
        let drive =
            validated_verbatim_wide(&wide(r"C:\temp\длинный"), true).expect("absolute drive path");
        assert_eq!(
            String::from_utf16_lossy(&drive),
            "\\\\?\\C:\\temp\\длинный\0"
        );

        let unc = validated_verbatim_wide(&wide(r"\\server\share\file"), true)
            .expect("absolute UNC path");
        assert_eq!(
            String::from_utf16_lossy(&unc),
            "\\\\?\\UNC\\server\\share\\file\0"
        );

        let verbatim = validated_verbatim_wide(&wide(r"\\?\C:\already"), true)
            .expect("existing verbatim path");
        assert_eq!(String::from_utf16_lossy(&verbatim), "\\\\?\\C:\\already\0");
    }

    #[test]
    fn relative_and_nul_paths_fail_closed() {
        assert_eq!(
            validated_verbatim_wide(&wide(r"relative\file"), false)
                .expect_err("relative path")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            validated_verbatim_wide(&[b'C' as u16, b':' as u16, 0], true)
                .expect_err("NUL path")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}
