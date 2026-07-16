use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

use super::anchored::AnchoredDir;
use super::{
    AUDIT_GIT_AUTHOR_EMAIL, AUDIT_GIT_AUTHOR_NAME, AUDIT_GIT_COMMIT_MESSAGE,
    MAX_AUDIT_GIT_METADATA_ENTRIES, SNAPSHOT_FILE_NAME,
};

const GIT_DIR: &str = ".git";
const HEAD_FILE: &str = "HEAD";
const CONFIG_FILE: &str = "config";
const OBJECTS_DIR: &str = "objects";
const REFS_DIR: &str = "refs";
const HEADS_DIR: &str = "heads";
const TAGS_DIR: &str = "tags";
const PACKED_REFS_FILE: &str = "packed-refs";
const MAX_HEAD_BYTES: u64 = 4096;
const MAX_REF_BYTES: u64 = 4096;
const MAX_PACKED_REFS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_COMMIT_BYTES: u64 = 1024 * 1024;
const MAX_TREE_BYTES: u64 = 32 * 1024 * 1024;

pub(super) fn commit_snapshot(root: &AnchoredDir) -> io::Result<()> {
    let repository = LocalGitAudit::open_or_initialize(root)?;
    repository.commit_snapshot()
}

struct LocalGitAudit<'root> {
    root: &'root AnchoredDir,
    git: AnchoredDir,
    objects: AnchoredDir,
    refs: AnchoredDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeadTarget {
    Symbolic(Vec<String>),
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeadState {
    target: HeadTarget,
    commit: Option<gix::hash::ObjectId>,
}

impl<'root> LocalGitAudit<'root> {
    fn open_or_initialize(root: &'root AnchoredDir) -> io::Result<Self> {
        let (git, fresh) = match root.child_dir_optional(GIT_DIR) {
            Ok(Some(git)) => (git, false),
            Ok(None) => (root.child_dir(GIT_DIR, true)?, true),
            Err(error) => {
                return Err(git_error(
                    "open existing repository",
                    format!(
                        "local git audit metadata is not a directory: {}: {error}",
                        root.logical_path().join(GIT_DIR).display()
                    ),
                ));
            }
        };

        if fresh {
            initialize_repository(&git)?;
        }
        let objects = git.child_dir(OBJECTS_DIR, false)?;
        let refs = git.child_dir(REFS_DIR, false)?;
        refs.child_dir(HEADS_DIR, false)?;
        git.open_regular(HEAD_FILE)?;
        git.open_regular(CONFIG_FILE)?;
        Ok(Self {
            root,
            git,
            objects,
            refs,
        })
    }

    fn commit_snapshot(&self) -> io::Result<()> {
        let head = self.read_head()?;
        let (mut tree, base_tree_id) = self.read_base_tree(head.commit)?;
        let blob_id = self.write_snapshot_blob()?;
        upsert_snapshot_entry(&mut tree, blob_id);
        let tree_id = self.write_object(&tree)?;
        if head.commit.is_some() && tree_id == base_tree_id {
            return Ok(());
        }

        let signature = gix::actor::Signature {
            name: AUDIT_GIT_AUTHOR_NAME.into(),
            email: AUDIT_GIT_AUTHOR_EMAIL.into(),
            time: gix::date::Time::now_utc(),
        };
        let commit = gix::objs::Commit {
            tree: tree_id,
            parents: head.commit.into_iter().collect(),
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: AUDIT_GIT_COMMIT_MESSAGE.into(),
            extra_headers: Vec::new(),
        };
        let commit_id = self.write_object(&commit)?;
        self.update_head(&head, commit_id)
    }

    fn read_head(&self) -> io::Result<HeadState> {
        let head = read_bounded(self.git.open_regular(HEAD_FILE)?, MAX_HEAD_BYTES, "HEAD")?;
        let head = trim_line(&head);
        if let Some(reference) = head.strip_prefix(b"ref: ") {
            let reference = std::str::from_utf8(reference).map_err(|_| {
                git_error("read HEAD", "symbolic reference name is not valid UTF-8")
            })?;
            let components = validate_head_reference(reference)?;
            let commit = self.read_reference(&components)?;
            Ok(HeadState {
                target: HeadTarget::Symbolic(components),
                commit,
            })
        } else {
            Ok(HeadState {
                target: HeadTarget::Detached,
                commit: Some(parse_object_id(head, "HEAD")?),
            })
        }
    }

    fn read_reference(&self, components: &[String]) -> io::Result<Option<gix::hash::ObjectId>> {
        let (directory, name) = traverse_parent(&self.refs, &components[1..], false)?;
        if let Some(file) = directory.open_regular_optional(name)? {
            let value = read_bounded(file, MAX_REF_BYTES, "loose reference")?;
            return Ok(Some(parse_object_id(trim_line(&value), "loose reference")?));
        }
        self.read_packed_reference(&components.join("/"))
    }

    fn read_packed_reference(&self, reference: &str) -> io::Result<Option<gix::hash::ObjectId>> {
        let Some(file) = self.git.open_regular_optional(PACKED_REFS_FILE)? else {
            return Ok(None);
        };
        let packed = read_bounded(file, MAX_PACKED_REFS_BYTES, PACKED_REFS_FILE)?;
        for line in packed.split(|byte| *byte == b'\n') {
            if line.is_empty() || matches!(line[0], b'#' | b'^') {
                continue;
            }
            let Some(space) = line.iter().position(|byte| *byte == b' ') else {
                return Err(git_error("read packed references", "malformed line"));
            };
            if &line[space + 1..] == reference.as_bytes() {
                return Ok(Some(parse_object_id(&line[..space], PACKED_REFS_FILE)?));
            }
        }
        Ok(None)
    }

    fn read_base_tree(
        &self,
        parent: Option<gix::hash::ObjectId>,
    ) -> io::Result<(gix::objs::Tree, gix::hash::ObjectId)> {
        let Some(parent) = parent else {
            return Ok((
                gix::objs::Tree::empty(),
                gix::hash::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            ));
        };
        let commit_bytes = self.read_loose_object(parent, MAX_COMMIT_BYTES)?;
        let commit = match decode_object(&commit_bytes, parent)? {
            gix::objs::ObjectRef::Commit(commit) => commit,
            _ => return Err(git_error("read HEAD commit", "HEAD is not a commit object")),
        };
        let tree_id = commit.tree();
        let tree_bytes = self.read_loose_object(tree_id, MAX_TREE_BYTES)?;
        let tree = match decode_object(&tree_bytes, tree_id)? {
            gix::objs::ObjectRef::Tree(tree) => tree.to_owned(),
            _ => {
                return Err(git_error(
                    "read HEAD tree",
                    "commit tree is not a tree object",
                ))
            }
        };
        if tree.entries.len() > MAX_AUDIT_GIT_METADATA_ENTRIES {
            return Err(git_error(
                "read HEAD tree",
                "tree exceeds audit metadata entry limit",
            ));
        }
        Ok((tree, tree_id))
    }

    fn read_loose_object(&self, id: gix::hash::ObjectId, max_bytes: u64) -> io::Result<Vec<u8>> {
        let hex = id.to_string();
        let fanout = match self.objects.child_dir(&hex[..2], false) {
            Ok(fanout) => fanout,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return self.read_packed_object_read_only(id, max_bytes);
            }
            Err(error) => return Err(error),
        };
        let file = match fanout.open_regular(&hex[2..]) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return self.read_packed_object_read_only(id, max_bytes);
            }
            Err(error) => return Err(error),
        };
        let bytes = inflate_bounded(file, max_bytes, &format!("object {id}"))?;
        verify_loose_bytes(&bytes, id)?;
        Ok(bytes)
    }

    /// Packed objects are read through gix only after the exact id came from
    /// anchored HEAD/ref data. This fallback is strictly read-only; the bytes
    /// are reconstructed as a loose object and hash-verified before use.
    fn read_packed_object_read_only(
        &self,
        id: gix::hash::ObjectId,
        max_bytes: u64,
    ) -> io::Result<Vec<u8>> {
        let repository = gix::open(self.root.logical_path())
            .map_err(|error| git_error("open packed-object reader", error))?;
        let object = repository
            .find_object(id)
            .map_err(|error| git_error(format!("read packed object {id}"), error))?;
        let header = gix::objs::encode::loose_header(object.kind, object.data.len() as u64);
        let total = header
            .len()
            .checked_add(object.data.len())
            .ok_or_else(|| git_error("read packed object", "object size overflow"))?;
        if total as u64 > max_bytes {
            return Err(git_error(
                "read packed object",
                "object exceeds the bounded audit limit",
            ));
        }
        let mut loose = Vec::with_capacity(total);
        loose.extend_from_slice(&header);
        loose.extend_from_slice(&object.data);
        verify_loose_bytes(&loose, id)?;
        Ok(loose)
    }

    fn write_snapshot_blob(&self) -> io::Result<gix::hash::ObjectId> {
        let mut snapshot = self.root.open_regular(SNAPSHOT_FILE_NAME)?;
        let size = snapshot.metadata()?.len();
        let id = hash_stream(gix::objs::Kind::Blob, size, &mut snapshot)?;
        snapshot.seek(SeekFrom::Start(0))?;
        self.write_stream_object(gix::objs::Kind::Blob, size, &mut snapshot, id)?;
        Ok(id)
    }

    fn write_object(&self, object: &dyn gix::objs::WriteTo) -> io::Result<gix::hash::ObjectId> {
        let capacity = usize::try_from(object.size())
            .unwrap_or(2 * 1024 * 1024)
            .min(MAX_TREE_BYTES as usize);
        let mut data = Vec::with_capacity(capacity);
        object
            .write_to(&mut data)
            .map_err(|error| git_error("encode object", error))?;
        let id = gix::objs::compute_hash(gix::hash::Kind::Sha1, object.kind(), &data)
            .map_err(|error| git_error("hash object", error))?;
        self.write_stream_object(object.kind(), data.len() as u64, &mut data.as_slice(), id)?;
        Ok(id)
    }

    fn write_stream_object(
        &self,
        kind: gix::objs::Kind,
        size: u64,
        input: &mut dyn Read,
        id: gix::hash::ObjectId,
    ) -> io::Result<()> {
        let hex = id.to_string();
        let fanout = self.objects.child_dir(&hex[..2], true)?;
        let object_name = &hex[2..];
        let expected_loose_bytes = size
            .checked_add(128)
            .ok_or_else(|| git_error("write object", "object size overflow"))?;
        if let Some(existing) = fanout.open_regular_optional(object_name)? {
            return verify_loose_stream(existing, id, expected_loose_bytes);
        }

        let temporary_name = format!(
            ".tmp-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        );
        let temporary = fanout.create_new_regular(&temporary_name)?;
        let write_result = (|| {
            let header = gix::objs::encode::loose_header(kind, size);
            let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
            hasher.update(&header);
            let buffered = BufWriter::new(&temporary);
            let mut compressed = gix::features::zlib::stream::deflate::Write::new(buffered);
            compressed.write_all(&header)?;

            let mut remaining = size;
            let mut buffer = [0_u8; 32 * 1024];
            while remaining > 0 {
                let wanted = usize::try_from(remaining.min(buffer.len() as u64))
                    .expect("bounded object chunk fits usize");
                let read = input.read(&mut buffer[..wanted])?;
                if read == 0 {
                    return Err(git_error("write object", "object stream ended early"));
                }
                hasher.update(&buffer[..read]);
                compressed.write_all(&buffer[..read])?;
                remaining -= read as u64;
            }
            let mut extra = [0_u8; 1];
            if input.read(&mut extra)? != 0 {
                return Err(git_error(
                    "write object",
                    "object stream exceeded declared size",
                ));
            }
            let actual = hasher
                .try_finalize()
                .map_err(|error| git_error("hash object stream", error))?;
            if actual != id {
                return Err(git_error(
                    "write object",
                    "object stream changed after its id was computed",
                ));
            }
            compressed.flush()?;
            let mut buffered = compressed.into_inner();
            buffered.flush()?;
            drop(buffered);
            temporary.sync_all()
        })();
        if let Err(error) = write_result {
            drop(temporary);
            let _ = fanout.remove_regular(&temporary_name);
            return Err(error);
        }

        match fanout.publish_regular_no_replace(&temporary, &temporary_name, object_name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                drop(temporary);
                let _ = fanout.remove_regular(&temporary_name);
                let existing = fanout.open_regular(object_name)?;
                verify_loose_stream(existing, id, expected_loose_bytes)
            }
            Err(error) => {
                drop(temporary);
                let _ = fanout.remove_regular(&temporary_name);
                Err(error)
            }
        }
    }

    fn update_head(&self, expected: &HeadState, commit_id: gix::hash::ObjectId) -> io::Result<()> {
        let (directory, name) = match &expected.target {
            HeadTarget::Detached => (self.git.clone(), HEAD_FILE),
            HeadTarget::Symbolic(components) => {
                traverse_parent(&self.refs, &components[1..], true)?
            }
        };
        let lock_name = format!("{name}.lock");
        let mut lock = directory.create_new_regular(&lock_name)?;

        // A symbolic HEAD needs both locks: the target ref lock provides the
        // compare-and-swap boundary, while HEAD.lock prevents a normal Git
        // checkout from switching the selected branch after that comparison.
        let head_guard = if matches!(expected.target, HeadTarget::Symbolic(_)) {
            match self.git.create_new_regular("HEAD.lock") {
                Ok(guard) => Some(guard),
                Err(error) => {
                    drop(lock);
                    let _ = directory.remove_regular(&lock_name);
                    return Err(error);
                }
            }
        } else {
            None
        };

        let current = match self.read_head() {
            Ok(current) => current,
            Err(error) => {
                drop(lock);
                let _ = directory.remove_regular(&lock_name);
                if let Some(guard) = head_guard {
                    drop(guard);
                    let _ = self.git.remove_regular("HEAD.lock");
                }
                return Err(error);
            }
        };
        if &current != expected {
            drop(lock);
            let _ = directory.remove_regular(&lock_name);
            if let Some(guard) = head_guard {
                drop(guard);
                let _ = self.git.remove_regular("HEAD.lock");
            }
            return Err(git_error(
                "update HEAD",
                "reference changed before its lock was acquired",
            ));
        }
        let update_result = (|| {
            writeln!(lock, "{commit_id}")?;
            lock.sync_all()?;
            directory.replace_regular(&lock, &lock_name, name)
        })();
        drop(lock);
        if update_result.is_err() {
            let _ = directory.remove_regular(&lock_name);
        }
        let guard_cleanup = if let Some(guard) = head_guard {
            drop(guard);
            self.git.remove_regular("HEAD.lock")
        } else {
            Ok(())
        };
        update_result.and(guard_cleanup)
    }
}

fn upsert_snapshot_entry(tree: &mut gix::objs::Tree, blob_id: gix::hash::ObjectId) {
    if let Some(entry) = tree
        .entries
        .iter_mut()
        .find(|entry| entry.filename.as_slice() == SNAPSHOT_FILE_NAME.as_bytes())
    {
        entry.mode = gix::objs::tree::EntryKind::Blob.into();
        entry.oid = blob_id;
    } else {
        tree.entries.push(gix::objs::tree::Entry {
            mode: gix::objs::tree::EntryKind::Blob.into(),
            filename: SNAPSHOT_FILE_NAME.into(),
            oid: blob_id,
        });
    }
    tree.entries.sort();
}

fn validate_head_reference(reference: &str) -> io::Result<Vec<String>> {
    let components = reference.split('/').collect::<Vec<_>>();
    if components.len() < 3 || components[0] != REFS_DIR || components[1] != HEADS_DIR {
        return Err(git_error(
            "read HEAD",
            "symbolic HEAD must target refs/heads",
        ));
    }
    for component in &components {
        if component.is_empty()
            || component == &"."
            || component == &".."
            || !component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(git_error(
                "read HEAD",
                "symbolic reference contains an unsafe component",
            ));
        }
    }
    Ok(components.into_iter().map(str::to_string).collect())
}

fn traverse_parent<'a>(
    root: &AnchoredDir,
    components: &'a [String],
    create: bool,
) -> io::Result<(AnchoredDir, &'a str)> {
    let (name, parents) = components
        .split_last()
        .ok_or_else(|| git_error("resolve reference", "reference has no components"))?;
    let mut directory = root.clone();
    for component in parents {
        directory = directory.child_dir(component, create)?;
    }
    Ok((directory, name))
}

fn parse_object_id(bytes: &[u8], source: &str) -> io::Result<gix::hash::ObjectId> {
    if bytes.len() != gix::hash::Kind::Sha1.len_in_hex() {
        return Err(git_error(
            format!("read {source}"),
            "expected one full SHA-1 object id",
        ));
    }
    gix::hash::ObjectId::from_hex(bytes).map_err(|error| git_error(format!("read {source}"), error))
}

fn trim_line(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn read_bounded(mut file: File, max_bytes: u64, source: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(git_error(
            format!("read {source}"),
            "file exceeds the bounded audit limit",
        ));
    }
    Ok(bytes)
}

struct InflateReader<R> {
    inner: R,
    decompressor: gix::features::zlib::Decompress,
}

impl<R: io::BufRead> InflateReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            decompressor: gix::features::zlib::Decompress::new(),
        }
    }
}

impl<R: io::BufRead> Read for InflateReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        gix::features::zlib::stream::inflate::read(&mut self.inner, &mut self.decompressor, output)
    }
}

fn inflate_bounded(file: File, max_bytes: u64, source: &str) -> io::Result<Vec<u8>> {
    let mut inflated = InflateReader::new(BufReader::new(file));
    let mut bytes = Vec::new();
    inflated
        .by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(git_error(
            format!("inflate {source}"),
            "loose object exceeds the bounded audit limit",
        ));
    }
    Ok(bytes)
}

fn verify_loose_bytes(bytes: &[u8], expected: gix::hash::ObjectId) -> io::Result<()> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(bytes);
    let actual = hasher
        .try_finalize()
        .map_err(|error| git_error("verify loose object", error))?;
    if actual == expected {
        Ok(())
    } else {
        Err(git_error(
            "verify loose object",
            format!("object id mismatch: expected {expected}, found {actual}"),
        ))
    }
}

fn verify_loose_stream(
    file: File,
    expected: gix::hash::ObjectId,
    max_bytes: u64,
) -> io::Result<()> {
    let mut inflated = InflateReader::new(BufReader::new(file));
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    let mut total = 0_u64;
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = inflated.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| git_error("verify loose object", "object size overflow"))?;
        if total > max_bytes {
            return Err(git_error(
                "verify loose object",
                "object exceeds its expected bounded size",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hasher
        .try_finalize()
        .map_err(|error| git_error("verify loose object", error))?;
    if actual == expected {
        Ok(())
    } else {
        Err(git_error(
            "verify loose object",
            format!("object id mismatch: expected {expected}, found {actual}"),
        ))
    }
}

fn hash_stream(
    kind: gix::objs::Kind,
    size: u64,
    input: &mut dyn Read,
) -> io::Result<gix::hash::ObjectId> {
    let header = gix::objs::encode::loose_header(kind, size);
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(&header);
    let mut remaining = size;
    let mut buffer = [0_u8; 32 * 1024];
    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded hash chunk fits usize");
        let read = input.read(&mut buffer[..wanted])?;
        if read == 0 {
            return Err(git_error("hash object stream", "object stream ended early"));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if input.read(&mut extra)? != 0 {
        return Err(git_error(
            "hash object stream",
            "object stream exceeded declared size",
        ));
    }
    hasher
        .try_finalize()
        .map_err(|error| git_error("hash object stream", error))
}

fn decode_object(
    bytes: &[u8],
    expected: gix::hash::ObjectId,
) -> io::Result<gix::objs::ObjectRef<'_>> {
    gix::objs::ObjectRef::from_loose(bytes, gix::hash::Kind::Sha1)
        .map_err(|error| git_error(format!("decode object {expected}"), error))
}

fn git_error(context: impl std::fmt::Display, error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("local git audit could not {context}: {error}"))
}

fn initialize_repository(git: &AnchoredDir) -> io::Result<()> {
    let objects = git.child_dir(OBJECTS_DIR, true)?;
    objects.child_dir("info", true)?;
    objects.child_dir("pack", true)?;
    let refs = git.child_dir(REFS_DIR, true)?;
    refs.child_dir(HEADS_DIR, true)?;
    refs.child_dir(TAGS_DIR, true)?;
    write_new_file(git, HEAD_FILE, b"ref: refs/heads/main\n")?;
    write_new_file(
        git,
        CONFIG_FILE,
        format!(
            "[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = false\n\tlogallrefupdates = false\n[user]\n\tname = {AUDIT_GIT_AUTHOR_NAME}\n\temail = {AUDIT_GIT_AUTHOR_EMAIL}\n"
        )
        .as_bytes(),
    )
}

fn write_new_file(directory: &AnchoredDir, name: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = directory.create_new_regular(name)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        directory.sync()
    })();
    if result.is_err() {
        drop(file);
        let _ = directory.remove_regular(name);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn temp_workspace(name: &str) -> (PathBuf, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test time should follow the epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!(
            "aopmem-anchored-git-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&workspace).expect("test workspace should create");
        let audit = workspace.join("audit-git");
        (workspace, audit)
    }

    fn assert_git_fsck(audit: &Path) {
        let output = Command::new("git")
            .current_dir(audit)
            .args(["fsck", "--full"])
            .output()
            .expect("git fsck should start");
        assert!(
            output.status.success(),
            "git fsck should pass: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn outside_fixture(workspace: &Path, name: &str) -> (PathBuf, PathBuf) {
        let outside = workspace.join(format!("outside-{name}"));
        std::fs::create_dir(&outside).expect("outside directory should create");
        let sentinel = outside.join("sentinel.txt");
        std::fs::write(&sentinel, b"outside git sentinel\n")
            .expect("outside sentinel should write");
        (outside, sentinel)
    }

    #[cfg(unix)]
    fn assert_outside_untouched(outside: &Path, sentinel: &Path) {
        assert_eq!(
            std::fs::read(sentinel).expect("outside sentinel should read"),
            b"outside git sentinel\n"
        );
        assert_eq!(
            std::fs::read_dir(outside)
                .expect("outside directory should list")
                .count(),
            1,
            "anchored Git must not create any outside entry"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_repository_survives_git_directory_swap_without_outside_writes() {
        let (workspace, audit) = temp_workspace("git-dir-swap");
        let root = AnchoredDir::open_or_create_audit_root(&audit).expect("root should anchor");
        write_new_file(&root, SNAPSHOT_FILE_NAME, b"first snapshot\n")
            .expect("snapshot fixture should write");
        let repository =
            LocalGitAudit::open_or_initialize(&root).expect("repository should initialize");
        repository
            .commit_snapshot()
            .expect("first commit should write");
        std::fs::write(audit.join(SNAPSHOT_FILE_NAME), b"second snapshot\n")
            .expect("changed snapshot should write");

        let moved = audit.join(".git-moved");
        let (outside, sentinel) = outside_fixture(&workspace, "git-dir");
        std::fs::rename(audit.join(GIT_DIR), &moved).expect("git directory should move");
        symlink(&outside, audit.join(GIT_DIR)).expect("replacement git symlink should create");

        repository
            .commit_snapshot()
            .expect("open Git capability should stay on the moved original");
        assert_outside_untouched(&outside, &sentinel);

        std::fs::remove_file(audit.join(GIT_DIR)).expect("replacement symlink should remove");
        std::fs::rename(&moved, audit.join(GIT_DIR)).expect("git directory should restore");
        assert_git_fsck(&audit);
        drop(repository);
        drop(root);
        std::fs::remove_dir_all(workspace).expect("test workspace should remove");
    }

    #[cfg(unix)]
    #[test]
    fn open_object_store_survives_directory_swap_without_outside_writes() {
        let (workspace, audit) = temp_workspace("objects-swap");
        let root = AnchoredDir::open_or_create_audit_root(&audit).expect("root should anchor");
        write_new_file(&root, SNAPSHOT_FILE_NAME, b"first snapshot\n")
            .expect("snapshot fixture should write");
        let repository =
            LocalGitAudit::open_or_initialize(&root).expect("repository should initialize");
        repository
            .commit_snapshot()
            .expect("first commit should write");
        std::fs::write(audit.join(SNAPSHOT_FILE_NAME), b"second snapshot\n")
            .expect("changed snapshot should write");

        let objects = audit.join(GIT_DIR).join(OBJECTS_DIR);
        let moved = audit.join(GIT_DIR).join("objects-moved");
        let (outside, sentinel) = outside_fixture(&workspace, "objects");
        std::fs::rename(&objects, &moved).expect("objects directory should move");
        symlink(&outside, &objects).expect("replacement objects symlink should create");

        repository
            .commit_snapshot()
            .expect("open object capability should stay on the moved original");
        assert_outside_untouched(&outside, &sentinel);

        std::fs::remove_file(&objects).expect("replacement symlink should remove");
        std::fs::rename(&moved, &objects).expect("objects directory should restore");
        assert_git_fsck(&audit);
        drop(repository);
        drop(root);
        std::fs::remove_dir_all(workspace).expect("test workspace should remove");
    }

    #[cfg(unix)]
    #[test]
    fn open_ref_store_updates_original_after_directory_swap() {
        let (workspace, audit) = temp_workspace("refs-swap");
        let root = AnchoredDir::open_or_create_audit_root(&audit).expect("root should anchor");
        write_new_file(&root, SNAPSHOT_FILE_NAME, b"snapshot\n")
            .expect("snapshot fixture should write");
        let repository =
            LocalGitAudit::open_or_initialize(&root).expect("repository should initialize");
        repository
            .commit_snapshot()
            .expect("first commit should write");
        let expected = repository.read_head().expect("HEAD should read");
        let commit_id = expected.commit.expect("HEAD should have a commit");

        let refs = audit.join(GIT_DIR).join(REFS_DIR);
        let moved = audit.join(GIT_DIR).join("refs-moved");
        let (outside, sentinel) = outside_fixture(&workspace, "refs");
        std::fs::rename(&refs, &moved).expect("refs directory should move");
        symlink(&outside, &refs).expect("replacement refs symlink should create");

        repository
            .update_head(&expected, commit_id)
            .expect("open ref capability should update the moved original");
        assert_outside_untouched(&outside, &sentinel);

        std::fs::remove_file(&refs).expect("replacement symlink should remove");
        std::fs::rename(&moved, &refs).expect("refs directory should restore");
        assert_git_fsck(&audit);
        drop(repository);
        drop(root);
        std::fs::remove_dir_all(workspace).expect("test workspace should remove");
    }

    #[test]
    fn stale_expected_reference_is_rejected_and_lock_files_are_removed() {
        let (workspace, audit) = temp_workspace("reference-cas");
        let root = AnchoredDir::open_or_create_audit_root(&audit).expect("root should anchor");
        write_new_file(&root, SNAPSHOT_FILE_NAME, b"snapshot\n")
            .expect("snapshot fixture should write");
        let repository =
            LocalGitAudit::open_or_initialize(&root).expect("repository should initialize");
        repository
            .commit_snapshot()
            .expect("first commit should write");
        let expected = repository.read_head().expect("HEAD should read");
        let commit_id = expected.commit.expect("HEAD should have a commit");
        let changed = "1111111111111111111111111111111111111111\n";
        let reference = audit.join(".git/refs/heads/main");
        std::fs::write(&reference, changed).expect("reference fixture should change");

        let error = repository
            .update_head(&expected, commit_id)
            .expect_err("stale reference expectation must fail");
        assert!(error.to_string().contains("reference changed"));
        assert_eq!(
            std::fs::read_to_string(&reference).expect("changed reference should remain"),
            changed
        );
        assert!(!audit.join(".git/refs/heads/main.lock").exists());
        assert!(!audit.join(".git/HEAD.lock").exists());

        drop(repository);
        drop(root);
        std::fs::remove_dir_all(workspace).expect("test workspace should remove");
    }

    #[test]
    fn windows_anchor_source_has_no_delete_share_and_uses_handle_relative_rename() {
        let source = include_str!("anchored.rs");
        let audit_source = include_str!("mod.rs");
        assert!(source.contains("FILE_FLAG_OPEN_REPARSE_POINT"));
        assert!(source.contains("SetFileInformationByHandle"));
        assert!(source.contains("RootDirectory = parent.as_raw_handle()"));
        assert!(!source.contains("FILE_SHARE_DELETE"));
        assert!(source.contains(".ancestors()"));
        assert!(source.contains("ancestors.push(Arc::new(windows_open("));
        assert!(source.contains("ancestors.push(Arc::clone(&self.handle))"));
        assert!(source.contains("GetFileInformationByHandle"));
        assert!(source.contains("dwVolumeSerialNumber"));
        assert!(source.contains("nFileIndexHigh"));
        assert!(source.contains("nFileIndexLow"));

        let composite = audit_source
            .split_once("pub(crate) fn acquire_workspace_mutation_locks")
            .expect("composite lock function should exist")
            .1;
        let mutation_lock = composite
            .find("open_or_create_regular(mutation_lock_name)")
            .expect("mutation lock should use the workspace capability");
        let audit_child = composite
            .find("workspace\n        .child_dir_os")
            .expect("audit root should derive from the same workspace capability");
        let snapshot_lock = composite
            .find("SnapshotLock::acquire_anchored")
            .expect("snapshot lock should use the derived audit capability");
        assert!(mutation_lock < audit_child);
        assert!(audit_child < snapshot_lock);
    }
}
