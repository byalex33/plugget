use crate::{
    providers::{Artifact, PackageProvider, Project, Version},
    state::{self, Change, Config, Installed, Lock},
};
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeMap, path::Path};

pub fn compatible(version: &Version, config: &Config) -> Result<bool> {
    let (platform, minecraft) = config.server()?;
    Ok(version.minecraft.iter().any(|v| v == minecraft)
        && version
            .loaders
            .iter()
            .any(|l| platform.loaders().contains(&l.as_str()))
        && (version.channel == "release" || config.allow_prerelease.unwrap_or(false)))
}
pub fn select<'a>(
    versions: &'a [Version],
    config: &Config,
    requested: Option<&str>,
) -> Result<&'a Version> {
    config.server()?;
    let mut candidates: Vec<_> = versions
        .iter()
        .filter(|v| {
            compatible(v, config).unwrap_or(false)
                && requested.is_none_or(|r| v.id == r || v.number == r)
        })
        .collect();
    candidates.sort_by_key(|v| std::cmp::Reverse(v.published));
    if requested.is_some() && candidates.len() > 1 {
        if let Some(v) = candidates.iter().find(|v| Some(v.id.as_str()) == requested) {
            return Ok(v);
        }
        bail!("Version number is ambiguous; use an exact version ID");
    }
    candidates.first().copied().context("No compatible version found. Check the Minecraft version/platform; prereleases require --prerelease")
}
pub fn artifact(version: &Version) -> Result<&Artifact> {
    let primary: Vec<_> = version.artifacts.iter().filter(|a| a.primary).collect();
    let selected = match primary.as_slice() {
        [a] => *a,
        [] if version.artifacts.len() == 1 => &version.artifacts[0],
        _ => bail!("Version {} has no unambiguous plugin JAR", version.id),
    };
    state::safe_filename(&selected.filename)?;
    state::valid_hash(&selected.sha512)?;
    Ok(selected)
}
pub async fn resolve(
    provider: &impl PackageProvider,
    query: &str,
    choose: &dyn Fn(&[Project]) -> Result<usize>,
) -> Result<Project> {
    if let Some(project) = provider.project(query).await? {
        return Ok(project);
    }
    let candidates = provider.search(query, 20).await?;
    ensure!(
        !candidates.is_empty(),
        "No matching project found for '{query}'"
    );
    // Even a single fuzzy hit needs explicit selection: a typo is not authorization.
    let index = choose(&candidates)?;
    candidates
        .get(index)
        .cloned()
        .context("Invalid project selection")
}

#[derive(Clone, Debug)]
pub struct Planned {
    pub project: Project,
    pub version: Version,
    pub dependencies: BTreeMap<String, Option<String>>,
}

pub async fn plan(
    provider: &impl PackageProvider,
    project: Project,
    requested: Option<&str>,
    config: &Config,
    lock: &Lock,
) -> Result<Vec<Planned>> {
    let versions = provider.versions(&project.id).await?;
    let version = select(&versions, config, requested)?.clone();
    ensure!(
        version.project_id == project.id,
        "Provider returned a version for another project"
    );
    let mut pending = vec![(project, version, false)];
    let mut visiting = BTreeMap::<String, String>::new();
    let mut resolved = BTreeMap::<String, String>::new();
    let mut result = Vec::new();
    let mut dependencies = BTreeMap::<String, BTreeMap<String, Option<String>>>::new();
    while let Some((project, version, exiting)) = pending.pop() {
        ensure!(
            result.len() + pending.len() < 256,
            "Dependency graph exceeds 256 packages"
        );
        if exiting {
            visiting.remove(&project.id);
            resolved.insert(project.id.clone(), version.id.clone());
            let deps = dependencies.remove(&project.id).unwrap_or_default();
            result.push(Planned {
                project,
                version,
                dependencies: deps,
            });
            continue;
        }
        if let Some(existing) = resolved.get(&project.id) {
            ensure!(
                existing == &version.id,
                "Conflicting dependency versions for {}",
                project.slug
            );
            continue;
        }
        ensure!(
            !visiting.contains_key(&project.id),
            "Dependency loop involving {}",
            project.slug
        );
        ensure!(
            compatible(&version, config)?,
            "Required dependency {} is incompatible",
            project.slug
        );
        artifact(&version)?;
        visiting.insert(project.id.clone(), version.id.clone());
        pending.push((project.clone(), version.clone(), true));
        for dep in &version.dependencies {
            if dep.kind == "incompatible" {
                let conflict = lock.packages.values().any(|p| {
                    dep.project_id.as_ref().is_none_or(|id| id == &p.project_id)
                        && dep.version_id.as_ref().is_none_or(|id| id == &p.version_id)
                });
                ensure!(
                    !conflict,
                    "{} declares a conflict with an installed package",
                    project.slug
                );
            }
            if dep.kind != "required" {
                continue;
            }
            let (dep_project, dep_version) = if let Some(id) = &dep.version_id {
                let v = provider.version(id).await?;
                ensure!(
                    v.id == *id && dep.project_id.as_ref().is_none_or(|p| p == &v.project_id),
                    "Dependency identity mismatch"
                );
                let p = provider
                    .project(&v.project_id)
                    .await?
                    .context("Required dependency project missing")?;
                (p, v)
            } else if let Some(id) = &dep.project_id {
                let p = provider
                    .project(id)
                    .await?
                    .context("Required dependency project missing")?;
                let versions = provider.versions(id).await?;
                let v = if let Some(installed) = lock.packages.get(id) {
                    versions
                        .iter()
                        .find(|v| {
                            v.id == installed.version_id && compatible(v, config).unwrap_or(false)
                        })
                        .cloned()
                        .unwrap_or(select(&versions, config, None)?.clone())
                } else {
                    select(&versions, config, None)?.clone()
                };
                (p, v)
            } else {
                bail!(
                    "{} requires an external dependency without a Modrinth identity; automatic installation is unsafe",
                    project.slug
                );
            };
            ensure!(
                dep_project.id == dep_version.project_id,
                "Dependency project/version mismatch"
            );
            dependencies
                .entry(project.id.clone())
                .or_default()
                .insert(dep_project.id.clone(), dep.version_id.clone());
            pending.push((dep_project, dep_version, false));
        }
    }
    let selected: BTreeMap<_, _> = result
        .iter()
        .map(|p| (p.project.id.as_str(), p.version.id.as_str()))
        .collect();
    for p in &result {
        for d in &p.version.dependencies {
            if d.kind == "incompatible" {
                let conflict = selected.iter().any(|(id, v)| {
                    d.project_id.as_deref().is_none_or(|p| p == *id)
                        && d.version_id.as_deref().is_none_or(|x| x == *v)
                });
                ensure!(
                    !conflict,
                    "Dependency plan contains an incompatibility declared by {}",
                    p.project.slug
                );
            }
        }
    }
    Ok(result)
}

pub async fn install_plan(
    provider: &impl PackageProvider,
    root: &Path,
    before: &Lock,
    plan: &[Planned],
) -> Result<Vec<String>> {
    let changed: Vec<_> = plan
        .iter()
        .filter(|p| {
            before
                .packages
                .get(&p.project.id)
                .is_none_or(|old| old.version_id != p.version.id)
        })
        .collect();
    for p in plan {
        if let Some(old) = before.packages.get(&p.project.id) {
            state::verify_owned(root, old)?;
        }
    }
    if changed.is_empty() {
        return Ok(vec![]);
    }
    let mut after = before.clone();
    let directory = state::staging(root)?;
    let mut changes = vec![];
    for (i, p) in changed.iter().enumerate() {
        let a = artifact(&p.version)?;
        let installed = Installed {
            name: p.project.name.clone(),
            slug: p.project.slug.clone(),
            source: provider.name().into(),
            project_id: p.project.id.clone(),
            version_id: p.version.id.clone(),
            version_number: p.version.number.clone(),
            filename: a.filename.clone(),
            sha512: a.sha512.to_ascii_lowercase(),
            installed_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)?,
            dependencies: p.dependencies.keys().cloned().collect(),
            dependency_versions: p
                .dependencies
                .iter()
                .filter_map(|(id, v)| v.as_ref().map(|v| (id.clone(), v.clone())))
                .collect(),
            published: p
                .version
                .published
                .format(&time::format_description::well_known::Rfc3339)?,
        };
        after
            .packages
            .insert(p.project.id.clone(), installed.clone());
        changes.push(Change {
            old: before.packages.get(&p.project.id).cloned(),
            new: Some(installed),
            staged: format!("new-{i}.jar"),
            backup: format!("old-{i}.jar"),
        });
    }
    after.validate()?;
    for (p, c) in changed.iter().zip(&changes) {
        provider.download(artifact(&p.version)?, &directory.join(&c.staged)).await.with_context(|| format!("Could not download {} {}. Existing plugins were not changed. Retry; doctor reports retained staging files", p.project.name, p.version.number))?;
    }
    state::commit(root, before.clone(), after, &directory, changes)?;
    Ok(changed
        .iter()
        .map(|p| format!("{} {}", p.project.name, p.version.number))
        .collect())
}

pub fn remove(root: &Path, before: &Lock, query: &str) -> Result<String> {
    let old = before.find(query)?.clone();
    for p in before.packages.values() {
        ensure!(
            !p.dependencies.contains(&old.project_id),
            "{} requires {}; remove the dependent first",
            p.slug,
            old.slug
        );
    }
    state::verify_owned(root, &old)?;
    let mut after = before.clone();
    after.packages.remove(&old.project_id);
    let directory = state::staging(root)?;
    state::commit(
        root,
        before.clone(),
        after,
        &directory,
        vec![Change {
            old: Some(old.clone()),
            new: None,
            staged: "new-0.jar".into(),
            backup: "old-0.jar".into(),
        }],
    )?;
    Ok(old.name)
}
