use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::ffi::CString;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as UnixMetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle};

#[cfg(unix)]
const FILE_MODE: u32 = 0o600;
#[cfg(unix)]
const DIRECTORY_MODE: u32 = 0o700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilesystemIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
}

/// Opaque identity captured after workspace path validation.
///
/// Mutation code hands this value to the audit layer so a real-directory swap
/// between validation and snapshot locking fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkspaceIdentity(FilesystemIdentity);

impl WorkspaceIdentity {
    pub(crate) fn capture(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            let metadata = fs::metadata(path)?;
            Ok(Self(filesystem_identity(&metadata)))
        }
        #[cfg(windows)]
        {
            let directory = windows_open(path, FileOpenMode::ReadOnly, true)?;
            Self::from_file(&directory, path)
        }
    }

    fn from_file(file: &File, path: &Path) -> io::Result<Self> {
        Ok(Self(filesystem_identity_from_file(file, path)?))
    }
}

/// A directory capability. Its path is only a diagnostic label.
///
/// Unix operations are relative to `handle`. Windows retains every ancestor
/// handle without delete sharing, so validated components cannot be replaced.
#[derive(Debug, Clone)]
pub(crate) struct AnchoredDir {
    logical_path: PathBuf,
    handle: Arc<File>,
    ancestors: Vec<Arc<File>>,
}

impl AnchoredDir {
    #[cfg(unix)]
    pub(crate) fn raw_directory_fd(&self) -> std::os::fd::RawFd {
        self.handle.as_raw_fd()
    }

    #[cfg(test)]
    pub(super) fn open_or_create_audit_root(audit_root: &Path) -> io::Result<Self> {
        Self::open_or_create_audit_root_with_identity(audit_root, None)
    }

    pub(super) fn open_or_create_audit_root_with_identity(
        audit_root: &Path,
        expected_workspace: Option<WorkspaceIdentity>,
    ) -> io::Result<Self> {
        let workspace_root = audit_root.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "audit snapshot directory has no workspace parent",
            )
        })?;
        let name = audit_root
            .file_name()
            .ok_or_else(|| unsafe_name(audit_root))?;
        validate_component(name)?;

        let workspace = Self::open_workspace(workspace_root, expected_workspace)?;
        create_child_directory(&workspace.handle, workspace_root, name)?;
        let handle = open_child_directory(&workspace.handle, audit_root, name)?;
        let mut ancestors = workspace.ancestors.clone();
        ancestors.push(Arc::clone(&workspace.handle));
        Ok(Self {
            logical_path: audit_root.to_path_buf(),
            handle: Arc::new(handle),
            ancestors,
        })
    }

    pub(crate) fn open_workspace(
        workspace_root: &Path,
        expected_workspace: Option<WorkspaceIdentity>,
    ) -> io::Result<Self> {
        let (workspace, ancestors) = open_directory_chain(workspace_root)?;
        if let Some(expected) = expected_workspace {
            let actual = WorkspaceIdentity::from_file(&workspace, workspace_root)?;
            if actual != expected {
                return Err(unsafe_entry(
                    workspace_root,
                    "workspace identity changed after mutation path validation",
                ));
            }
        }
        Ok(Self {
            logical_path: workspace_root.to_path_buf(),
            handle: Arc::new(workspace),
            ancestors,
        })
    }

    pub(crate) fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    pub(crate) fn verify_logical_identity(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.logical_path)?;
        if metadata_is_reparse(&metadata) || metadata.file_type().is_symlink() || !metadata.is_dir()
        {
            return Err(unsafe_entry(
                &self.logical_path,
                "anchored directory path is not the opened real directory",
            ));
        }
        #[cfg(unix)]
        let path_identity = filesystem_identity(&metadata);
        #[cfg(windows)]
        let path_identity = WorkspaceIdentity::capture(&self.logical_path)?.0;
        let handle_identity = filesystem_identity_from_file(&self.handle, &self.logical_path)?;
        if path_identity != handle_identity {
            return Err(unsafe_entry(
                &self.logical_path,
                "anchored directory path identity changed",
            ));
        }
        Ok(())
    }

    /// Returns a path-free token for the opened directory identity.
    ///
    /// Upgrade recovery persists this token so a same-name directory swap is
    /// detected before any migration is resumed.
    pub(crate) fn stable_identity_token(&self) -> io::Result<String> {
        let identity = filesystem_identity_from_file(&self.handle, &self.logical_path)?;
        Ok(filesystem_identity_token(identity))
    }

    pub(super) fn child_dir(&self, name: &str, create: bool) -> io::Result<Self> {
        self.child_dir_os(OsStr::new(name), create)
    }

    pub(crate) fn child_dir_os(&self, name: &OsStr, create: bool) -> io::Result<Self> {
        validate_component(name)?;
        let path = self.logical_path.join(name);
        if create {
            create_child_directory(&self.handle, &self.logical_path, name)?;
        }
        let handle = open_child_directory(&self.handle, &path, name)?;
        let mut ancestors = self.ancestors.clone();
        ancestors.push(Arc::clone(&self.handle));
        Ok(Self {
            logical_path: path,
            handle: Arc::new(handle),
            ancestors,
        })
    }

    pub(crate) fn create_new_child_dir_os(&self, name: &OsStr) -> io::Result<Self> {
        validate_component(name)?;
        let path = self.logical_path.join(name);
        create_new_child_directory(&self.handle, &self.logical_path, name)?;
        match open_child_directory(&self.handle, &path, name) {
            Ok(handle) => {
                let mut ancestors = self.ancestors.clone();
                ancestors.push(Arc::clone(&self.handle));
                Ok(Self {
                    logical_path: path,
                    handle: Arc::new(handle),
                    ancestors,
                })
            }
            Err(error) => {
                let _ = remove_empty_child_directory(&self.handle, &path, name);
                Err(error)
            }
        }
    }

    pub(crate) fn remove_empty_child_dir_os(&self, name: &OsStr) -> io::Result<()> {
        validate_component(name)?;
        remove_empty_child_directory(&self.handle, &self.logical_path.join(name), name)
    }

    pub(super) fn child_dir_optional(&self, name: &str) -> io::Result<Option<Self>> {
        match self.child_dir(name, false) {
            Ok(directory) => Ok(Some(directory)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn open_regular(&self, name: &str) -> io::Result<File> {
        self.open_regular_os(OsStr::new(name))
    }

    pub(crate) fn open_regular_os(&self, name: &OsStr) -> io::Result<File> {
        self.open_regular_with_os(name, FileOpenMode::ReadOnly)
    }

    pub(super) fn open_regular_optional(&self, name: &str) -> io::Result<Option<File>> {
        match self.open_regular(name) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn open_regular_optional_os(&self, name: &OsStr) -> io::Result<Option<File>> {
        match self.open_regular_os(name) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn open_or_create_regular(&self, name: &str) -> io::Result<File> {
        self.open_regular_with(name, FileOpenMode::OpenOrCreate)
    }

    pub(crate) fn create_new_regular(&self, name: &str) -> io::Result<File> {
        self.open_regular_with(name, FileOpenMode::CreateNew)
    }

    pub(crate) fn create_new_regular_os(&self, name: &OsStr) -> io::Result<File> {
        self.open_regular_with_os(name, FileOpenMode::CreateNew)
    }

    pub(crate) fn open_regular_for_update_os(&self, name: &OsStr) -> io::Result<File> {
        self.open_regular_with_os(name, FileOpenMode::ReadWriteExisting)
    }

    /// Reopens one anchored child and confirms it still denotes `opened`.
    /// The file identity remains private to this module.
    pub(crate) fn regular_child_matches_open_file(
        &self,
        name: &OsStr,
        opened: &File,
    ) -> io::Result<bool> {
        let current = self.open_regular_os(name)?;
        same_file(opened, &current)
    }

    /// Returns a path-free identity token for one anchored regular child.
    pub(crate) fn regular_child_identity_token(&self, name: &OsStr) -> io::Result<String> {
        let file = self.open_regular_os(name)?;
        let identity = filesystem_identity_from_file(&file, &self.logical_path.join(name))?;
        Ok(filesystem_identity_token(identity))
    }

    fn open_regular_with(&self, name: &str, mode: FileOpenMode) -> io::Result<File> {
        self.open_regular_with_os(OsStr::new(name), mode)
    }

    fn open_regular_with_os(&self, name: &OsStr, mode: FileOpenMode) -> io::Result<File> {
        validate_component(name)?;
        open_child_regular(&self.handle, &self.logical_path.join(name), name, mode)
    }

    pub(super) fn validate_descendant_tree(&self, maximum_entries: usize) -> io::Result<()> {
        let mut pending = vec![self.clone()];
        let mut entries_seen = 0_usize;
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory.logical_path())? {
                let entry = entry?;
                entries_seen = entries_seen.checked_add(1).ok_or_else(|| {
                    unsafe_entry(directory.logical_path(), "entry count overflow")
                })?;
                if entries_seen > maximum_entries {
                    return Err(unsafe_entry(
                        self.logical_path(),
                        "metadata tree exceeds its bounded entry limit",
                    ));
                }
                let name = entry.file_name();
                validate_component(&name)?;
                let metadata = fs::symlink_metadata(entry.path())?;
                if metadata_is_reparse(&metadata) || metadata.file_type().is_symlink() {
                    return Err(unsafe_entry(&entry.path(), "links are forbidden"));
                }
                if metadata.is_dir() {
                    pending.push(directory.child_dir_os(&name, false)?);
                } else if metadata.is_file() {
                    directory.open_regular_os(&name)?;
                } else {
                    return Err(unsafe_entry(
                        &entry.path(),
                        "metadata entry is not a regular file or directory",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn remove_regular(&self, name: &str) -> io::Result<()> {
        self.remove_regular_os(OsStr::new(name))
    }

    pub(crate) fn remove_regular_os(&self, name: &OsStr) -> io::Result<()> {
        validate_component(name)?;
        remove_child_regular(&self.handle, &self.logical_path.join(name), name)
    }

    pub(crate) fn sync(&self) -> io::Result<()> {
        sync_directory_handle(&self.handle)
    }

    pub(crate) fn sync_for_publish(&self) -> io::Result<bool> {
        sync_directory_handle_strict(&self.handle)
    }
}

#[derive(Clone, Copy)]
enum FileOpenMode {
    ReadOnly,
    ReadWriteExisting,
    OpenOrCreate,
    CreateNew,
    #[cfg(windows)]
    DeleteExisting,
}

fn validate_component(name: &OsStr) -> io::Result<()> {
    let path = Path::new(name);
    let mut components = path.components();
    if name.is_empty()
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(unsafe_name(path));
    }
    Ok(())
}

fn unsafe_name(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("unsafe anchored audit path component: {}", path.display()),
    )
}

fn unsafe_entry(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("unsafe audit entry {}: {reason}", path.display()),
    )
}

fn validate_directory(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.is_dir() && !metadata_is_reparse(&metadata) {
        Ok(())
    } else {
        Err(unsafe_entry(path, "not a real directory"))
    }
}

fn validate_regular(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.is_file() && !metadata_is_reparse(&metadata) {
        Ok(())
    } else {
        Err(unsafe_entry(path, "not a real regular file"))
    }
}

fn same_file(left: &File, right: &File) -> io::Result<bool> {
    let left = filesystem_identity_from_file(left, Path::new("<open audit source>"))?;
    let right = filesystem_identity_from_file(right, Path::new("<anchored audit source>"))?;
    Ok(left == right)
}

#[cfg(unix)]
fn filesystem_identity_token(identity: FilesystemIdentity) -> String {
    format!("unix:{:016x}:{:016x}", identity.device, identity.inode)
}

#[cfg(windows)]
fn filesystem_identity_token(identity: FilesystemIdentity) -> String {
    format!(
        "windows:{:08x}:{:016x}",
        identity.volume_serial, identity.file_index
    )
}

#[cfg(unix)]
fn filesystem_identity(metadata: &fs::Metadata) -> FilesystemIdentity {
    FilesystemIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(unix)]
fn filesystem_identity_from_file(file: &File, _path: &Path) -> io::Result<FilesystemIdentity> {
    let metadata = file.metadata()?;
    Ok(filesystem_identity(&metadata))
}

#[cfg(windows)]
fn filesystem_identity_from_file(file: &File, _path: &Path) -> io::Result<FilesystemIdentity> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let queried = unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle() as *mut std::ffi::c_void,
            information.as_mut_ptr(),
        )
    };
    if queried == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetFileInformationByHandle succeeded and initialized the value.
    let information = unsafe { information.assume_init() };
    Ok(FilesystemIdentity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
    })
}

#[cfg(not(windows))]
fn metadata_is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn open_directory_chain(path: &Path) -> io::Result<(File, Vec<Arc<File>>)> {
    use std::os::unix::fs::OpenOptionsExt;

    if !path.is_absolute() {
        return Err(unsafe_entry(path, "audit workspace path must be absolute"));
    }
    let walk_path = trusted_unix_walk_path(path);
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(unix_directory_flags())
        .open(Path::new("/"))?;
    validate_directory(&file, Path::new("/"))?;

    let mut current_path = PathBuf::from("/");
    let mut saw_root = false;
    for component in walk_path.components() {
        match component {
            std::path::Component::RootDir if !saw_root => saw_root = true,
            std::path::Component::Normal(name) if saw_root => {
                current_path.push(name);
                file = open_child_directory(&file, &current_path, name)?;
            }
            _ => {
                return Err(unsafe_entry(
                    path,
                    "audit workspace path contains an unsafe component",
                ));
            }
        }
    }
    if !saw_root {
        return Err(unsafe_entry(path, "audit workspace path must be absolute"));
    }
    Ok((file, Vec::new()))
}

#[cfg(target_os = "linux")]
fn trusted_unix_walk_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(target_os = "macos")]
fn trusted_unix_walk_path(path: &Path) -> PathBuf {
    // macOS exposes these root-owned aliases by default. Following only these
    // fixed first components keeps ordinary /tmp and /var workspaces working;
    // every user-controlled later symlink is still rejected by openat.
    for (alias, target) in [("/var", "/private/var"), ("/tmp", "/private/tmp")] {
        if let Ok(remainder) = path.strip_prefix(alias) {
            return Path::new(target).join(remainder);
        }
    }
    path.to_path_buf()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn create_child_directory(parent: &File, _parent_path: &Path, name: &OsStr) -> io::Result<()> {
    let name = c_name(name)?;
    if system_mkdirat(parent.as_raw_fd(), name.as_ptr(), DIRECTORY_MODE) == 0 {
        sync_directory_handle(parent)
    } else {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn create_new_child_directory(parent: &File, _parent_path: &Path, name: &OsStr) -> io::Result<()> {
    let name = c_name(name)?;
    if system_mkdirat(parent.as_raw_fd(), name.as_ptr(), DIRECTORY_MODE) == 0 {
        if let Err(error) = sync_directory_handle(parent) {
            let _ = system_unlinkat(parent.as_raw_fd(), name.as_ptr(), remove_directory_flag());
            return Err(error);
        }
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn remove_empty_child_directory(parent: &File, _path: &Path, name: &OsStr) -> io::Result<()> {
    let name = c_name(name)?;
    if system_unlinkat(parent.as_raw_fd(), name.as_ptr(), remove_directory_flag()) == 0 {
        sync_directory_handle(parent)
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
const fn remove_directory_flag() -> i32 {
    libc::AT_REMOVEDIR
}

#[cfg(target_os = "linux")]
const fn remove_directory_flag() -> i32 {
    0x200
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn open_child_directory(parent: &File, path: &Path, name: &OsStr) -> io::Result<File> {
    let name = c_name(name)?;
    let descriptor = system_openat(parent.as_raw_fd(), name.as_ptr(), unix_directory_flags(), 0);
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned one new owned descriptor.
    let file = unsafe { File::from_raw_fd(descriptor) };
    validate_directory(&file, path)?;
    Ok(file)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn open_child_regular(
    parent: &File,
    path: &Path,
    name: &OsStr,
    mode: FileOpenMode,
) -> io::Result<File> {
    let name = c_name(name)?;
    let flags = match mode {
        FileOpenMode::ReadOnly => unix_read_flags(),
        FileOpenMode::ReadWriteExisting => unix_write_flags(),
        FileOpenMode::OpenOrCreate => unix_write_flags() | unix_o_create(),
        FileOpenMode::CreateNew => unix_write_flags() | unix_o_create() | unix_o_exclusive(),
    };
    let descriptor = system_openat(parent.as_raw_fd(), name.as_ptr(), flags, FILE_MODE);
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned one new owned descriptor.
    let file = unsafe { File::from_raw_fd(descriptor) };
    validate_regular(&file, path)?;
    Ok(file)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn remove_child_regular(parent: &File, path: &Path, name: &OsStr) -> io::Result<()> {
    let _entry = open_child_regular(parent, path, name, FileOpenMode::ReadWriteExisting)?;
    let name = c_name(name)?;
    if system_unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) == 0 {
        sync_directory_handle(parent)
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn sync_directory_handle(directory: &File) -> io::Result<()> {
    directory.sync_all()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn sync_directory_handle_strict(directory: &File) -> io::Result<bool> {
    directory.sync_all().map(|()| true)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "audit name contains NUL"))
}

#[cfg(target_os = "macos")]
fn unix_directory_flags() -> i32 {
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(target_os = "macos")]
fn unix_read_flags() -> i32 {
    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(target_os = "macos")]
fn unix_write_flags() -> i32 {
    libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(target_os = "macos")]
fn unix_o_create() -> i32 {
    libc::O_CREAT
}

#[cfg(target_os = "macos")]
fn unix_o_exclusive() -> i32 {
    libc::O_EXCL
}

#[cfg(target_os = "macos")]
fn system_openat(parent: i32, name: *const i8, flags: i32, mode: u32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { libc::openat(parent, name, flags, mode as libc::c_uint) }
}

#[cfg(target_os = "macos")]
fn system_mkdirat(parent: i32, name: *const i8, mode: u32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { libc::mkdirat(parent, name, mode as libc::mode_t) }
}

#[cfg(target_os = "macos")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { libc::unlinkat(parent, name, flags) }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn macos_tmp_alias_is_accepted_by_trusted_component_walk() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test time should follow epoch")
            .as_nanos();
        let workspace = Path::new("/tmp").join(format!(
            "aopmem-audit-tmp-alias-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&workspace).expect("/tmp workspace should create");
        let audit = workspace.join("audit-git");

        let root = AnchoredDir::open_or_create_audit_root(&audit)
            .expect("root-owned /tmp alias should be accepted");
        assert!(audit.is_dir());

        drop(root);
        fs::remove_dir_all(workspace).expect("/tmp workspace should remove");
    }
}

#[cfg(target_os = "linux")]
fn unix_directory_flags() -> i32 {
    0o2_000_000 | 0o200_000 | 0o400_000
}

#[cfg(target_os = "linux")]
fn unix_read_flags() -> i32 {
    0o2_000_000 | 0o400_000
}

#[cfg(target_os = "linux")]
fn unix_write_flags() -> i32 {
    0o2 | 0o2_000_000 | 0o400_000
}

#[cfg(target_os = "linux")]
fn unix_o_create() -> i32 {
    0o100
}

#[cfg(target_os = "linux")]
fn unix_o_exclusive() -> i32 {
    0o200
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "openat"]
    fn linux_openat(parent: i32, name: *const i8, flags: i32, ...) -> i32;
    #[link_name = "mkdirat"]
    fn linux_mkdirat(parent: i32, name: *const i8, mode: u32) -> i32;
    #[link_name = "unlinkat"]
    fn linux_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32;
    #[link_name = "renameat"]
    fn linux_renameat(
        source_parent: i32,
        source: *const i8,
        destination_parent: i32,
        destination: *const i8,
    ) -> i32;
    #[link_name = "linkat"]
    fn linux_linkat(
        source_parent: i32,
        source: *const i8,
        destination_parent: i32,
        destination: *const i8,
        flags: i32,
    ) -> i32;
}

#[cfg(target_os = "linux")]
fn system_openat(parent: i32, name: *const i8, flags: i32, mode: u32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { linux_openat(parent, name, flags, mode) }
}

#[cfg(target_os = "linux")]
fn system_mkdirat(parent: i32, name: *const i8, mode: u32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { linux_mkdirat(parent, name, mode) }
}

#[cfg(target_os = "linux")]
fn system_unlinkat(parent: i32, name: *const i8, flags: i32) -> i32 {
    // SAFETY: name is live and NUL-terminated; parent is an open directory.
    unsafe { linux_unlinkat(parent, name, flags) }
}

#[cfg(target_os = "linux")]
fn system_renameat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe { linux_renameat(source_parent, source, destination_parent, destination) }
}

#[cfg(target_os = "linux")]
fn system_linkat(
    source_parent: i32,
    source: *const i8,
    destination_parent: i32,
    destination: *const i8,
    flags: i32,
) -> i32 {
    // SAFETY: names are NUL-terminated and parents are open directories.
    unsafe {
        linux_linkat(
            source_parent,
            source,
            destination_parent,
            destination,
            flags,
        )
    }
}

#[cfg(windows)]
fn open_directory_chain(path: &Path) -> io::Result<(File, Vec<Arc<File>>)> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows audit workspace path must be absolute",
        ));
    }
    let mut components = path
        .ancestors()
        .filter(|ancestor| ancestor.is_absolute())
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    components.reverse();
    let final_path = components.pop().ok_or_else(|| unsafe_name(path))?;
    let mut ancestors = Vec::with_capacity(components.len());
    for component in components {
        ancestors.push(Arc::new(windows_open(
            &component,
            FileOpenMode::ReadOnly,
            true,
        )?));
    }
    let file = windows_open(&final_path, FileOpenMode::ReadOnly, true)?;
    Ok((file, ancestors))
}

#[cfg(windows)]
fn create_child_directory(_parent: &File, parent_path: &Path, name: &OsStr) -> io::Result<()> {
    let path = parent_path.join(name);
    match fs::create_dir(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn create_new_child_directory(_parent: &File, parent_path: &Path, name: &OsStr) -> io::Result<()> {
    fs::create_dir(parent_path.join(name))
}

#[cfg(windows)]
fn remove_empty_child_directory(_parent: &File, path: &Path, _name: &OsStr) -> io::Result<()> {
    let directory = windows_open(path, FileOpenMode::DeleteExisting, true)?;
    windows_mark_delete(&directory)
}

#[cfg(windows)]
fn open_child_directory(_parent: &File, path: &Path, _name: &OsStr) -> io::Result<File> {
    windows_open(path, FileOpenMode::ReadOnly, true)
}

#[cfg(windows)]
fn open_child_regular(
    _parent: &File,
    path: &Path,
    _name: &OsStr,
    mode: FileOpenMode,
) -> io::Result<File> {
    windows_open(path, mode, false)
}

#[cfg(windows)]
fn windows_open(path: &Path, mode: FileOpenMode, directory: bool) -> io::Result<File> {
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, CREATE_NEW, DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS, OPEN_EXISTING,
    };

    let path_wide = crate::windows_path::verbatim_wide_path(path)?;

    let (access, creation) = match mode {
        FileOpenMode::ReadOnly => (GENERIC_READ | FILE_READ_ATTRIBUTES, OPEN_EXISTING),
        FileOpenMode::ReadWriteExisting => (GENERIC_READ | GENERIC_WRITE | DELETE, OPEN_EXISTING),
        FileOpenMode::OpenOrCreate => (GENERIC_READ | GENERIC_WRITE | DELETE, OPEN_ALWAYS),
        FileOpenMode::CreateNew => (GENERIC_READ | GENERIC_WRITE | DELETE, CREATE_NEW),
        FileOpenMode::DeleteExisting => (DELETE | FILE_READ_ATTRIBUTES, OPEN_EXISTING),
    };
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    // SAFETY: path_wide is NUL-terminated and all pointers remain live.
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            creation,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned one new owned handle.
    let file = unsafe { File::from_raw_handle(handle as _) };
    if directory {
        validate_directory(&file, path)?;
    } else {
        validate_regular(&file, path)?;
    }
    Ok(file)
}

#[cfg(windows)]
fn remove_child_regular(_parent: &File, path: &Path, _name: &OsStr) -> io::Result<()> {
    let file = windows_open(path, FileOpenMode::DeleteExisting, false)?;
    windows_mark_delete(&file)
}

#[cfg(windows)]
fn windows_mark_delete(file: &File) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: file owns a live handle and disposition has the required layout.
    let deleted = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as _,
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if deleted == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn sync_directory_handle(directory: &File) -> io::Result<()> {
    sync_directory_handle_strict(directory).map(|_| ())
}

#[cfg(windows)]
fn sync_directory_handle_strict(directory: &File) -> io::Result<bool> {
    use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;

    // SAFETY: directory owns a live handle.
    let flushed = unsafe { FlushFileBuffers(directory.as_raw_handle() as _) };
    if flushed != 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if matches!(error.raw_os_error(), Some(1 | 5)) {
        // Some local filesystems do not support flushing directory handles.
        // The shared publish boundary reports that durability is unconfirmed.
        Ok(false)
    } else {
        Err(error)
    }
}
