use plugget::{
    minecraft::{self, Platform},
    packages,
    providers::{Artifact, Dependency, Version},
    state::{self, Change, Config, Installed, Lock},
};
use sha2::{Digest, Sha512};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("plugget-test-")
        .tempdir()
        .unwrap()
        .keep()
}
fn server() -> PathBuf {
    let root = directory();
    fs::create_dir(root.join(".plugget")).unwrap();
    fs::create_dir(root.join("plugins")).unwrap();
    root
}
fn config() -> Config {
    Config {
        platform: Some(Platform::Paper),
        minecraft: Some("1.21.11".into()),
        allow_prerelease: None,
    }
}
fn version(id: &str, day: u8, channel: &str, minecraft: &str) -> Version {
    Version {
        id: id.into(),
        project_id: "project".into(),
        number: id.into(),
        published: time::OffsetDateTime::parse(
            &format!("2026-01-{day:02}T00:00:00Z"),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        channel: channel.into(),
        minecraft: vec![minecraft.into()],
        loaders: vec!["bukkit".into()],
        dependencies: vec![],
        artifacts: vec![Artifact {
            filename: "plugin.jar".into(),
            url: "https://cdn.modrinth.com/test.jar".into(),
            sha512: "a".repeat(128),
            size: 10,
            primary: true,
        }],
    }
}
fn installed(id: &str, filename: &str, bytes: &[u8]) -> Installed {
    Installed {
        name: id.into(),
        slug: id.into(),
        source: "modrinth".into(),
        project_id: id.into(),
        version_id: "v1".into(),
        version_number: "1".into(),
        filename: filename.into(),
        sha512: format!("{:x}", Sha512::digest(bytes)),
        installed_at: "2026-01-01T00:00:00Z".into(),
        published: "2026-01-01T00:00:00Z".into(),
        dependencies: vec![],
        dependency_versions: BTreeMap::new(),
    }
}
fn save(root: &Path, lock: &Lock) {
    state::atomic_write(
        &root.join(".plugget/lock.json"),
        &serde_json::to_vec(lock).unwrap(),
    )
    .unwrap();
}
fn recycle_for(root: &Path, path: &Path) -> anyhow::Result<()> {
    let bin = root.join("test-recycle-bin");
    fs::create_dir_all(&bin)?;
    let name = format!(
        "{}-{}",
        fs::read_dir(&bin)?.count(),
        path.file_name().unwrap().to_string_lossy()
    );
    fs::rename(path, bin.join(name))?;
    Ok(())
}

#[test]
fn newest_compatible_stable_and_explicit_prerelease() {
    let versions = vec![
        version("old", 1, "release", "1.21.11"),
        version("incompatible", 3, "release", "1.22"),
        version("beta", 4, "beta", "1.21.11"),
        version("stable", 2, "release", "1.21.11"),
    ];
    assert_eq!(
        packages::select(&versions, &config(), None).unwrap().id,
        "stable"
    );
    assert!(packages::select(&versions, &config(), Some("beta")).is_err());
    let mut c = config();
    c.allow_prerelease = Some(true);
    assert_eq!(packages::select(&versions, &c, None).unwrap().id, "beta");
    assert_eq!(
        packages::select(&versions, &c, Some("old")).unwrap().id,
        "old"
    );
    c.minecraft = Some("1.21.1".into());
    assert!(packages::select(&versions, &c, None).is_err());
}
#[test]
fn ambiguous_files_and_version_numbers() {
    let mut v = version("one", 1, "release", "1.21.11");
    let mut other = v.clone();
    other.id = "two".into();
    assert!(packages::select(&[v.clone(), other], &config(), Some("one")).is_ok());
    v.artifacts.push(v.artifacts[0].clone());
    assert!(packages::artifact(&v).is_err());
    for a in &mut v.artifacts {
        a.primary = false;
    }
    assert!(packages::artifact(&v).is_err());
    v.artifacts.pop();
    assert!(packages::artifact(&v).is_ok());
}
#[test]
fn malicious_filenames_rejected_cross_platform() {
    for name in [
        "../evil.jar",
        "..\\evil.jar",
        "C:evil.jar",
        "/evil.jar",
        ".hidden.jar",
        "CON.jar",
        "com1.jar",
        "NUL.jar",
        "x\n.jar",
        "x?.jar",
        "x.jar ",
        "x.zip",
        "a/b.jar",
    ] {
        assert!(state::safe_filename(name).is_err(), "{name}");
    }
    for name in ["LuckPerms-Bukkit-5.4.1.jar", "a+b (1).jar"] {
        assert!(state::safe_filename(name).is_ok());
    }
}
#[test]
fn lock_roundtrip_schema_ownership_and_dependencies() {
    let mut lock = Lock::default();
    lock.packages
        .insert("a".into(), installed("a", "a.jar", b"a"));
    assert_eq!(
        serde_json::from_slice::<Lock>(&serde_json::to_vec(&lock).unwrap()).unwrap(),
        lock
    );
    lock.validate().unwrap();
    lock.packages
        .insert("b".into(), installed("b", "A.jar", b"b"));
    assert!(lock.validate().is_err());
    lock.packages.get_mut("b").unwrap().filename = "b.jar".into();
    lock.packages
        .get_mut("a")
        .unwrap()
        .dependencies
        .push("missing".into());
    assert!(lock.validate().is_err());
    lock.schema = 2;
    assert!(lock.validate().is_err());
}
#[test]
fn atomic_replacement_and_process_lock() {
    let root = server();
    let path = root.join(".plugget/lock.json");
    state::atomic_write(&path, b"first").unwrap();
    state::atomic_write(&path, b"second").unwrap();
    assert_eq!(fs::read(path).unwrap(), b"second");
    let guard = state::Guard::acquire(&root).unwrap();
    assert!(state::Guard::acquire(&root).is_err());
    drop(guard);
    assert!(state::Guard::acquire(&root).is_ok());
}
#[test]
fn modified_managed_file_and_unmanaged_removal_are_refused() {
    let root = server();
    fs::write(root.join("plugins/private.jar"), b"private").unwrap();
    let lock = Lock::default();
    assert!(packages::remove(&root, &lock, "private").is_err());
    assert_eq!(
        fs::read(root.join("plugins/private.jar")).unwrap(),
        b"private"
    );
    let p = installed("p", "private.jar", b"other");
    assert!(state::verify_owned(&root, &p).is_err());
}
#[test]
fn transaction_commits_new_and_recycles_owned_old_only() {
    let root = server();
    fs::write(root.join("plugins/old.jar"), b"old").unwrap();
    fs::write(root.join("plugins/private.jar"), b"private").unwrap();
    let old = installed("p", "old.jar", b"old");
    let new = installed("p", "new.jar", b"new");
    let mut before = Lock::default();
    before.packages.insert("p".into(), old.clone());
    save(&root, &before);
    let mut after = before.clone();
    after.packages.insert("p".into(), new.clone());
    let staging = state::staging(&root).unwrap();
    fs::write(staging.join("new-0.jar"), b"new").unwrap();
    state::commit_with(
        &root,
        before,
        after.clone(),
        &staging,
        vec![Change {
            old: Some(old),
            new: Some(new),
            staged: "new-0.jar".into(),
            backup: "old-0.jar".into(),
        }],
        &|p| recycle_for(&root, p),
    )
    .unwrap();
    assert_eq!(Lock::load(&root).unwrap(), after);
    assert!(!root.join("plugins/old.jar").exists());
    assert_eq!(fs::read(root.join("plugins/new.jar")).unwrap(), b"new");
    assert_eq!(
        fs::read(root.join("plugins/private.jar")).unwrap(),
        b"private"
    );
}
#[test]
fn unmanaged_collision_preserves_lock_and_files() {
    let root = server();
    fs::write(root.join("plugins/new.jar"), b"private").unwrap();
    let before = Lock::default();
    save(&root, &before);
    let new = installed("p", "new.jar", b"new");
    let mut after = before.clone();
    after.packages.insert("p".into(), new.clone());
    let staging = state::staging(&root).unwrap();
    fs::write(staging.join("new-0.jar"), b"new").unwrap();
    assert!(
        state::commit_with(
            &root,
            before.clone(),
            after,
            &staging,
            vec![Change {
                old: None,
                new: Some(new),
                staged: "new-0.jar".into(),
                backup: "old-0.jar".into()
            }],
            &|p| recycle_for(&root, p)
        )
        .is_err()
    );
    assert_eq!(Lock::load(&root).unwrap(), before);
    assert_eq!(fs::read(root.join("plugins/new.jar")).unwrap(), b"private");
}
#[test]
fn interrupted_transaction_rolls_back_on_next_run() {
    let root = server();
    let old = installed("p", "same.jar", b"old");
    let new = installed("p", "same.jar", b"new");
    let mut before = Lock::default();
    before.packages.insert("p".into(), old.clone());
    save(&root, &before);
    let mut after = before.clone();
    after.packages.insert("p".into(), new.clone());
    let staging = state::staging(&root).unwrap();
    fs::write(staging.join("old-0.jar"), b"old").unwrap();
    fs::write(root.join("plugins/same.jar"), b"new").unwrap();
    let journal = serde_json::json!({"before":before,"after":after,"directory":staging.file_name().unwrap().to_str().unwrap(),"changes":[Change{old:Some(old),new:Some(new),staged:"new-0.jar".into(),backup:"old-0.jar".into()}]});
    state::atomic_write(
        &root.join(".plugget/transaction.json"),
        &serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    state::recover_with(&root, &|p| recycle_for(&root, p)).unwrap();
    assert_eq!(Lock::load(&root).unwrap(), before);
    assert_eq!(fs::read(root.join("plugins/same.jar")).unwrap(), b"old");
}
#[test]
fn dependency_prevents_removal() {
    let root = server();
    let mut lock = Lock::default();
    let dep = installed("dep", "dep.jar", b"dep");
    let mut main = installed("main", "main.jar", b"main");
    main.dependencies.push("dep".into());
    lock.packages.insert("main".into(), main);
    lock.packages.insert("dep".into(), dep);
    assert!(
        packages::remove(&root, &lock, "dep")
            .unwrap_err()
            .to_string()
            .contains("requires")
    );
}
#[test]
fn detection_uses_metadata_and_refuses_conflicting_versions() {
    let root = directory();
    fs::write(root.join("server.properties"), b"").unwrap();
    fs::write(root.join("paper-1.21.11-5.jar"), b"").unwrap();
    let d = minecraft::detect(&root).unwrap();
    assert_eq!(d.platform, Some(Platform::Paper));
    assert_eq!(d.minecraft.as_deref(), Some("1.21.11"));
    fs::write(root.join("paper-1.21.10-6.jar"), b"").unwrap();
    let d = minecraft::detect(&root).unwrap();
    assert!(d.minecraft.is_none());
    assert!(!d.warnings.is_empty());
}
#[test]
fn jar_metadata_and_duplicate_name_inspection() {
    use std::io::Write;
    let root = directory();
    let path = root.join("plugin.jar");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    zip.start_file("plugin.yml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"name: TestPlugin\nversion: 1\n").unwrap();
    zip.finish().unwrap();
    assert_eq!(minecraft::plugin_name(&path).as_deref(), Some("testplugin"));
}

struct Fake {
    versions: BTreeMap<String, Vec<Version>>,
}
impl plugget::providers::PackageProvider for Fake {
    fn name(&self) -> &'static str {
        "modrinth"
    }
    async fn search(&self, _: &str, _: u8) -> anyhow::Result<Vec<plugget::providers::Project>> {
        Ok(vec![])
    }
    async fn project(&self, id: &str) -> anyhow::Result<Option<plugget::providers::Project>> {
        Ok(Some(plugget::providers::Project {
            id: id.into(),
            slug: id.into(),
            name: id.into(),
            description: String::new(),
            url: String::new(),
            authors: vec![],
        }))
    }
    async fn versions(&self, id: &str) -> anyhow::Result<Vec<Version>> {
        Ok(self.versions.get(id).cloned().unwrap_or_default())
    }
    async fn version(&self, id: &str) -> anyhow::Result<Version> {
        self.versions
            .values()
            .flatten()
            .find(|v| v.id == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Missing version"))
    }
    async fn download(&self, _: &Artifact, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("Not used")
    }
}
#[tokio::test]
async fn dependency_loops_optional_and_pinned_conflicts() {
    use plugget::providers::PackageProvider;
    let mut a = version("a1", 1, "release", "1.21.11");
    a.project_id = "a".into();
    let mut b = version("b1", 1, "release", "1.21.11");
    b.project_id = "b".into();
    a.dependencies.push(Dependency {
        project_id: Some("b".into()),
        version_id: None,
        kind: "required".into(),
    });
    b.dependencies.push(Dependency {
        project_id: Some("a".into()),
        version_id: None,
        kind: "required".into(),
    });
    let mut fake = Fake {
        versions: BTreeMap::from([("a".into(), vec![a]), ("b".into(), vec![b])]),
    };
    let project = fake.project("a").await.unwrap().unwrap();
    assert!(
        packages::plan(&fake, project.clone(), None, &config(), &Lock::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("loop")
    );
    fake.versions.get_mut("b").unwrap()[0].dependencies[0].kind = "optional".into();
    let plan = packages::plan(&fake, project, None, &config(), &Lock::default())
        .await
        .unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].project.id, "b");
    fake.versions.get_mut("a").unwrap()[0].dependencies[0].version_id = Some("b1".into());
    let project = fake.project("a").await.unwrap().unwrap();
    let plan = packages::plan(&fake, project, None, &config(), &Lock::default())
        .await
        .unwrap();
    assert_eq!(plan[1].dependencies["b"].as_deref(), Some("b1"));
}

#[test]
fn same_filename_update_and_committed_cleanup_retry() {
    let root = server();
    fs::write(root.join("plugins/plugin.jar"), b"old").unwrap();
    let old = installed("p", "plugin.jar", b"old");
    let new = installed("p", "plugin.jar", b"new");
    let before = Lock {
        schema: 1,
        packages: BTreeMap::from([("p".into(), old.clone())]),
    };
    save(&root, &before);
    let after = Lock {
        schema: 1,
        packages: BTreeMap::from([("p".into(), new.clone())]),
    };
    let staging = state::staging(&root).unwrap();
    fs::write(staging.join("new-0.jar"), b"new").unwrap();
    let error = state::commit_with(
        &root,
        before,
        after.clone(),
        &staging,
        vec![Change {
            old: Some(old),
            new: Some(new),
            staged: "new-0.jar".into(),
            backup: "old-0.jar".into(),
        }],
        &|_| anyhow::bail!("Trash unavailable"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("committed"));
    assert_eq!(Lock::load(&root).unwrap(), after);
    assert_eq!(fs::read(root.join("plugins/plugin.jar")).unwrap(), b"new");
    assert_eq!(fs::read(staging.join("old-0.jar")).unwrap(), b"old");
    state::recover_with(&root, &|p| recycle_for(&root, p)).unwrap();
    assert!(!root.join(".plugget/transaction.json").exists());
    assert_eq!(Lock::load(&root).unwrap(), after);
}

#[test]
fn rollback_restores_old_even_when_trash_unavailable() {
    let root = server();
    let old = installed("p", "same.jar", b"old");
    let new = installed("p", "same.jar", b"new");
    let before = Lock {
        schema: 1,
        packages: BTreeMap::from([("p".into(), old.clone())]),
    };
    save(&root, &before);
    let after = Lock {
        schema: 1,
        packages: BTreeMap::from([("p".into(), new.clone())]),
    };
    let staging = state::staging(&root).unwrap();
    fs::write(staging.join("old-0.jar"), b"old").unwrap();
    fs::write(root.join("plugins/same.jar"), b"new").unwrap();
    let journal = serde_json::json!({"before":before,"after":after,"directory":staging.file_name().unwrap().to_str().unwrap(),"changes":[Change{old:Some(old),new:Some(new),staged:"new-0.jar".into(),backup:"old-0.jar".into()}]});
    state::atomic_write(
        &root.join(".plugget/transaction.json"),
        &serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    assert!(state::recover_with(&root, &|_| anyhow::bail!("Trash unavailable")).is_err());
    assert_eq!(fs::read(root.join("plugins/same.jar")).unwrap(), b"old");
    assert_eq!(Lock::load(&root).unwrap(), before);
    state::recover_with(&root, &|p| recycle_for(&root, p)).unwrap();
}

#[test]
fn incomplete_journal_cannot_claim_unmanaged_file() {
    let root = server();
    fs::write(root.join("plugins/private.jar"), b"private").unwrap();
    let staging = state::staging(&root).unwrap();
    let journal = serde_json::json!({"before":Lock::default(),"after":Lock::default(),"directory":staging.file_name().unwrap().to_str().unwrap(),"changes":[Change{old:Some(installed("p","private.jar",b"private")),new:None,staged:"new-0.jar".into(),backup:"old-0.jar".into()}]});
    state::atomic_write(
        &root.join(".plugget/transaction.json"),
        &serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    assert!(state::recover_with(&root, &|p| recycle_for(&root, p)).is_err());
    assert_eq!(
        fs::read(root.join("plugins/private.jar")).unwrap(),
        b"private"
    );
}

#[cfg(windows)]
#[test]
fn locked_second_plugin_rolls_back_first_plugin() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = server();
    let mut before = Lock::default();
    let mut after = Lock::default();
    let staging = state::staging(&root).unwrap();
    let mut changes = vec![];
    for (i, id) in ["a", "b"].iter().enumerate() {
        let filename = format!("{id}.jar");
        fs::write(root.join("plugins").join(&filename), b"old").unwrap();
        let old = installed(id, &filename, b"old");
        let new = installed(id, &filename, b"new");
        before.packages.insert(id.to_string(), old.clone());
        after.packages.insert(id.to_string(), new.clone());
        let staged = format!("new-{i}.jar");
        fs::write(staging.join(&staged), b"new").unwrap();
        changes.push(Change {
            old: Some(old),
            new: Some(new),
            staged,
            backup: format!("old-{i}.jar"),
        });
    }
    save(&root, &before);
    let _held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(root.join("plugins/b.jar"))
        .unwrap();
    assert!(
        state::commit_with(&root, before.clone(), after, &staging, changes, &|p| {
            recycle_for(&root, p)
        })
        .is_err()
    );
    assert_eq!(fs::read(root.join("plugins/a.jar")).unwrap(), b"old");
    assert_eq!(fs::read(root.join("plugins/b.jar")).unwrap(), b"old");
    assert_eq!(Lock::load(&root).unwrap(), before);
}

#[tokio::test]
async fn conflicting_exact_dependency_versions_and_external_dependencies_fail() {
    use plugget::providers::PackageProvider;
    let mut a = version("a1", 1, "release", "1.21.11");
    a.project_id = "a".into();
    let mut b = version("b1", 1, "release", "1.21.11");
    b.project_id = "b".into();
    let mut b2 = b.clone();
    b2.id = "b2".into();
    a.dependencies = vec![
        Dependency {
            project_id: Some("b".into()),
            version_id: Some("b1".into()),
            kind: "required".into(),
        },
        Dependency {
            project_id: Some("b".into()),
            version_id: Some("b2".into()),
            kind: "required".into(),
        },
    ];
    let mut fake = Fake {
        versions: BTreeMap::from([("a".into(), vec![a]), ("b".into(), vec![b, b2])]),
    };
    let project = fake.project("a").await.unwrap().unwrap();
    assert!(
        packages::plan(&fake, project.clone(), None, &config(), &Lock::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("Conflicting")
    );
    fake.versions.get_mut("a").unwrap()[0].dependencies = vec![Dependency {
        project_id: None,
        version_id: None,
        kind: "required".into(),
    }];
    assert!(
        packages::plan(&fake, project, None, &config(), &Lock::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("external")
    );
}

#[cfg(unix)]
#[test]
fn linked_plugin_directory_and_linked_jar_are_refused() {
    use std::os::unix::fs::symlink;
    let root = directory();
    let external = directory();
    symlink(&external, root.join("plugins")).unwrap();
    assert!(state::jars(&root).is_err());
    let root = server();
    fs::write(external.join("external.jar"), b"private").unwrap();
    symlink(external.join("external.jar"), root.join("plugins/link.jar")).unwrap();
    assert!(state::hash_file(&root.join("plugins/link.jar")).is_err());
}
