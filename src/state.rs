use crate::minecraft::{Platform, detect, valid_minecraft};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub platform: Option<Platform>,
    pub minecraft: Option<String>,
    pub allow_prerelease: Option<bool>,
}
impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        if root.join(".plugget").exists() {
            safe_directory(&root.join(".plugget"))?;
        }
        let mut config = Self::default();
        if let Some(dir) = directories::ProjectDirs::from("", "", "plugget") {
            let path = dir.config_dir().join("config.toml");
            if path.exists() {
                config = toml::from_str(&fs::read_to_string(&path)?)
                    .context("Invalid global Plugget configuration")?;
            }
        }
        let path = root.join(".plugget/config.toml");
        if path.exists() {
            regular(&path)?;
            let local: Self = toml::from_str(&fs::read_to_string(path)?)
                .context("Invalid server Plugget configuration")?;
            config.platform = local.platform.or(config.platform);
            config.minecraft = local.minecraft.or(config.minecraft);
            config.allow_prerelease = local.allow_prerelease.or(config.allow_prerelease);
        }
        let detected = detect(root)?;
        config.platform = config.platform.or(detected.platform);
        config.minecraft = config.minecraft.or(detected.minecraft);
        Ok(config)
    }
    pub fn server(&self) -> Result<(Platform, &str)> {
        let platform = self.platform.context(
            "Server platform unknown. Run plugget init --platform paper --minecraft <version>",
        )?;
        let minecraft = self.minecraft.as_deref().context("Minecraft version unknown. Run plugget init --minecraft <version> --platform <platform>")?;
        ensure!(
            valid_minecraft(minecraft),
            "Invalid Minecraft version in config"
        );
        Ok((platform, minecraft))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    pub name: String,
    pub slug: String,
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    pub sha512: String,
    pub installed_at: String,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub dependency_versions: BTreeMap<String, String>,
    pub published: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub schema: u32,
    pub packages: BTreeMap<String, Installed>,
}
impl Default for Lock {
    fn default() -> Self {
        Self {
            schema: 1,
            packages: BTreeMap::new(),
        }
    }
}
impl Lock {
    pub fn load(root: &Path) -> Result<Self> {
        if root.join(".plugget").exists() {
            safe_directory(&root.join(".plugget"))?;
        }
        let path = root.join(".plugget/lock.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        regular(&path)?;
        let lock: Self = serde_json::from_slice(&fs::read(path)?)
            .context("Invalid lock.json; restore a known-good backup before changing plugins")?;
        lock.validate()?;
        Ok(lock)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema == 1, "Unsupported lock schema {}", self.schema);
        let mut filenames = BTreeSet::new();
        for (id, p) in &self.packages {
            ensure!(
                id == &p.project_id && !id.is_empty() && !p.version_id.is_empty(),
                "Invalid package identity in lock"
            );
            ensure!(
                p.source == "modrinth",
                "Unsupported package source {}",
                p.source
            );
            safe_filename(&p.filename)?;
            valid_hash(&p.sha512)?;
            ensure!(
                filenames.insert(p.filename.to_ascii_lowercase()),
                "Multiple packages own the same filename"
            );
            for dep in &p.dependencies {
                ensure!(
                    self.packages.contains_key(dep),
                    "Missing dependency {dep} in lock"
                );
            }
            for (dep, version) in &p.dependency_versions {
                ensure!(
                    self.packages
                        .get(dep)
                        .is_some_and(|p| &p.version_id == version),
                    "Pinned dependency {dep} does not match {version}"
                );
            }
        }
        Ok(())
    }
    pub fn find(&self, query: &str) -> Result<&Installed> {
        let matches: Vec<_> = self
            .packages
            .values()
            .filter(|p| p.project_id == query || p.slug.eq_ignore_ascii_case(query))
            .collect();
        ensure!(
            matches.len() == 1,
            "Package '{query}' is not uniquely managed by Plugget; use plugget list"
        );
        Ok(matches[0])
    }
}

pub fn safe_filename(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 200 && name.to_ascii_lowercase().ends_with(".jar"),
        "Unsafe JAR filename: {name:?}"
    );
    ensure!(
        !name.starts_with('.')
            && !name.ends_with([' ', '.'])
            && !name
                .chars()
                .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c)),
        "Unsafe JAR filename: {name:?}"
    );
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&stem.as_str()),
        "Reserved filename: {name}"
    );
    Ok(())
}
pub fn valid_hash(hash: &str) -> Result<()> {
    ensure!(
        hash.len() == 128 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid SHA512 digest"
    );
    Ok(())
}
pub fn regular(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !is_link(&meta),
        "Refusing non-regular file: {}",
        path.display()
    );
    Ok(())
}
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
pub fn safe_directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !is_link(&meta),
        "Refusing linked or non-directory path: {}",
        path.display()
    );
    Ok(())
}
pub fn hash_file(path: &Path) -> Result<String> {
    regular(path)?;
    let mut file = File::open(path)?;
    let mut hash = Sha512::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn verify_owned(root: &Path, p: &Installed) -> Result<()> {
    safe_filename(&p.filename)?;
    safe_directory(&root.join("plugins"))?;
    ensure!(
        hash_file(&root.join("plugins").join(&p.filename))?.eq_ignore_ascii_case(&p.sha512),
        "Managed file {} was modified; no changes made. Restore it or investigate with plugget doctor",
        p.filename
    );
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent")?;
    safe_directory(parent)?;
    if path.exists() {
        regular(path)?;
    }
    let (mut file, temporary) = tempfile::Builder::new()
        .prefix("write-")
        .suffix(".tmp")
        .tempfile_in(parent)?
        .keep()?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
        .context("Atomic metadata replacement failed; original metadata is intact")?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub struct Guard {
    _file: File,
}
impl Guard {
    pub fn acquire(root: &Path) -> Result<Self> {
        let dir = root.join(".plugget");
        if !dir.exists() {
            fs::create_dir(&dir)?;
        }
        safe_directory(&dir)?;
        let path = dir.join("process.lock");
        if path.exists() {
            regular(&path)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        fs2::FileExt::try_lock_exclusive(&file)
            .context("Another Plugget process is currently modifying this server")?;
        Ok(Self { _file: file })
    }
}

pub fn jars(root: &Path) -> Result<Vec<String>> {
    let dir = root.join("plugins");
    if !dir.exists() {
        return Ok(vec![]);
    }
    safe_directory(&dir)?;
    let mut jars = vec![];
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_ascii_lowercase().ends_with(".jar") {
            jars.push(name);
        }
    }
    jars.sort();
    Ok(jars)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub old: Option<Installed>,
    pub new: Option<Installed>,
    pub staged: String,
    pub backup: String,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    before: Lock,
    after: Lock,
    directory: String,
    changes: Vec<Change>,
}
impl Journal {
    fn validate(&self) -> Result<()> {
        self.before.validate()?;
        self.after.validate()?;
        ensure!(
            self.directory.starts_with("transaction-")
                && self
                    .directory
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Unsafe transaction directory"
        );
        let mut expected = self.before.clone();
        let mut ids = BTreeSet::new();
        for (i, c) in self.changes.iter().enumerate() {
            ensure!(
                c.staged == format!("new-{i}.jar") && c.backup == format!("old-{i}.jar"),
                "Invalid transaction staging names"
            );
            let id = c
                .old
                .as_ref()
                .or(c.new.as_ref())
                .context("Empty transaction change")?
                .project_id
                .clone();
            ensure!(ids.insert(id.clone()), "Duplicate transaction identity");
            ensure!(
                self.before.packages.get(&id) == c.old.as_ref(),
                "Journal ownership mismatch"
            );
            if let Some(new) = &c.new {
                ensure!(new.project_id == id, "Transaction identity changed");
                expected.packages.insert(id, new.clone());
            } else {
                expected.packages.remove(&id);
            }
        }
        ensure!(
            expected == self.after,
            "Transaction does not match proposed lock"
        );
        Ok(())
    }
}

pub fn staging(root: &Path) -> Result<PathBuf> {
    Ok(tempfile::Builder::new()
        .prefix("transaction-")
        .tempdir_in(root.join(".plugget"))?
        .keep())
}
pub fn recycle(path: &Path) -> Result<()> {
    trash::delete(path).with_context(|| {
        format!(
            "Could not move {} to the OS Recycle Bin; retained for recovery",
            path.display()
        )
    })
}

/// Journal is durable before moving any live files. Lock replacement is the commit point.
pub fn commit(
    root: &Path,
    before: Lock,
    after: Lock,
    directory: &Path,
    changes: Vec<Change>,
) -> Result<()> {
    commit_with(root, before, after, directory, changes, &recycle)
}
pub fn commit_with(
    root: &Path,
    before: Lock,
    after: Lock,
    directory: &Path,
    changes: Vec<Change>,
    recycler: &dyn Fn(&Path) -> Result<()>,
) -> Result<()> {
    ensure!(
        !root.join(".plugget/transaction.json").exists(),
        "Pending transaction; run a mutating command to recover first"
    );
    before.validate()?;
    after.validate()?;
    safe_directory(directory)?;
    safe_directory(&root.join("plugins"))?;
    ensure!(
        directory.parent() == Some(root.join(".plugget").as_path()),
        "Staging directory is outside server metadata"
    );
    ensure!(
        Lock::load(root)? == before,
        "Lock state changed; operation aborted"
    );
    for change in &changes {
        safe_filename(&change.staged)?;
        safe_filename(&change.backup)?;
        ensure!(
            !directory.join(&change.backup).try_exists()?,
            "Backup path is occupied"
        );
        if let Some(old) = &change.old {
            verify_owned(root, old)?;
        }
        if let Some(new) = &change.new {
            safe_filename(&new.filename)?;
            ensure!(
                hash_file(&directory.join(&change.staged))? == new.sha512,
                "Staged checksum mismatch"
            );
            for existing in jars(root)? {
                if existing.eq_ignore_ascii_case(&new.filename) {
                    ensure!(
                        change.old.as_ref().is_some_and(|p| p.filename == existing),
                        "Refusing to overwrite existing file {existing}"
                    );
                }
            }
        }
    }
    let journal = Journal {
        before,
        after,
        directory: directory
            .file_name()
            .context("Missing staging name")?
            .to_string_lossy()
            .into_owned(),
        changes,
    };
    let path = root.join(".plugget/transaction.json");
    journal.validate()?;
    atomic_write(&path, &serde_json::to_vec_pretty(&journal)?)?;
    let result = (|| -> Result<()> {
        for c in &journal.changes {
            if let Some(old) = &c.old {
                fs::rename(
                    root.join("plugins").join(&old.filename),
                    directory.join(&c.backup),
                )?;
            }
            if let Some(new) = &c.new {
                // hard_link is an atomic no-clobber publication on the same filesystem.
                fs::hard_link(
                    directory.join(&c.staged),
                    root.join("plugins").join(&new.filename),
                )?;
            }
        }
        atomic_write(
            &root.join(".plugget/lock.json"),
            &serde_json::to_vec_pretty(&journal.after)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        recover_with(root, recycler).context(
            "Rollback needs attention; keep .plugget/transaction.json and run plugget doctor",
        )?;
        return Err(error)
            .context("Transaction failed; previous plugin files and lock state restored");
    }
    recover_with(root, recycler).context(
        "Changes committed, but Recycle Bin cleanup failed; rerun the command to finish cleanup",
    )
}
pub fn recover(root: &Path) -> Result<()> {
    recover_with(root, &recycle)
}
pub fn recover_with(root: &Path, recycler: &dyn Fn(&Path) -> Result<()>) -> Result<()> {
    let path = root.join(".plugget/transaction.json");
    if !path.exists() {
        return Ok(());
    }
    regular(&path)?;
    let journal: Journal = serde_json::from_slice(&fs::read(&path)?)
        .context("Invalid transaction journal; manual recovery required")?;
    journal.validate()?;
    safe_directory(&root.join("plugins"))?;
    for c in &journal.changes {
        safe_filename(&c.staged)?;
        safe_filename(&c.backup)?;
        if let Some(p) = &c.old {
            ensure!(
                journal.before.packages.get(&p.project_id) == Some(p),
                "Journal ownership mismatch"
            );
        }
        if let Some(p) = &c.new {
            ensure!(
                journal.after.packages.get(&p.project_id) == Some(p),
                "Journal ownership mismatch"
            );
        }
    }
    ensure!(
        journal.directory.starts_with("transaction-")
            && !journal.directory.contains(['/', '\\', ':', '.']),
        "Unsafe transaction directory"
    );
    let directory = root.join(".plugget").join(&journal.directory);
    let current = Lock::load(root)?;
    ensure!(
        current == journal.before || current == journal.after,
        "Lock differs from transaction; manual recovery required"
    );
    if current != journal.after && directory.exists() {
        safe_directory(&directory)?;
        for (i, c) in journal.changes.iter().enumerate().rev() {
            safe_filename(&c.staged)?;
            safe_filename(&c.backup)?;
            if let Some(new) = &c.new {
                safe_filename(&new.filename)?;
                let target = root.join("plugins").join(&new.filename);
                let old_moved = c.old.is_none() || directory.join(&c.backup).exists();
                if old_moved && target.exists() && hash_file(&target)? == new.sha512 {
                    let rollback = directory.join(format!("rollback-{i}.jar"));
                    ensure!(
                        !rollback.try_exists()?,
                        "Rollback path occupied; manual recovery required"
                    );
                    fs::rename(&target, rollback)?;
                }
            }
            if let Some(old) = &c.old {
                safe_filename(&old.filename)?;
                let backup = directory.join(&c.backup);
                if backup.exists() {
                    ensure!(
                        hash_file(&backup)? == old.sha512,
                        "Backup checksum mismatch"
                    );
                    let target = root.join("plugins").join(&old.filename);
                    ensure!(
                        !target.exists(),
                        "Recovery target occupied: {}",
                        target.display()
                    );
                    fs::rename(backup, target)?;
                }
            }
        }
    }
    if current != journal.after {
        for c in &journal.changes {
            if let Some(old) = &c.old {
                verify_owned(root, old)?;
            } else if let Some(new) = &c.new {
                ensure!(
                    !root.join("plugins").join(&new.filename).try_exists()?,
                    "New file still present after rollback"
                );
            }
        }
    }
    if directory.exists() {
        safe_directory(&directory)?;
        recycler(&directory)?;
    }
    recycler(&path)?;
    Ok(())
}

pub fn ensure_initialized(root: &Path) -> Result<()> {
    if !root.join(".plugget/config.toml").is_file() {
        bail!("Server is not initialized. Run plugget init first");
    }
    safe_directory(&root.join(".plugget"))?;
    safe_directory(&root.join("plugins"))
}
