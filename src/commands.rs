use crate::{
    cli::{Cli, Command},
    minecraft, packages,
    providers::{PackageProvider, Project, modrinth::Modrinth},
    state::{self, Config, Guard, Lock},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, IsTerminal, Write},
    path::Path,
};

pub struct Report {
    pub data: Value,
    pub text: String,
    pub code: i32,
}
fn report(data: Value, text: String) -> Report {
    Report {
        data,
        text,
        code: 0,
    }
}
fn clean(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}
fn interactive(cli: &Cli) -> bool {
    !cli.json && io::stdin().is_terminal() && io::stderr().is_terminal()
}
fn confirm(cli: &Cli, prompt: &str) -> Result<()> {
    if cli.yes {
        return Ok(());
    }
    ensure!(
        interactive(cli),
        "Confirmation required: {prompt}. Review and rerun with --yes"
    );
    eprint!("{} [y/N] ", clean(prompt));
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    ensure!(
        ["y", "yes"].contains(&answer.trim().to_ascii_lowercase().as_str()),
        "Cancelled; no plugin changes made"
    );
    Ok(())
}
fn choose(cli: &Cli, candidates: &[Project]) -> Result<usize> {
    let matches = candidates
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}: {} ({})", i + 1, clean(&p.name), clean(&p.slug)))
        .collect::<Vec<_>>()
        .join("\n");
    ensure!(
        interactive(cli),
        "No exact project match. Choose an exact slug or ID from these possible matches:\n{matches}"
    );
    eprintln!("Select a project:\n{matches}");
    eprint!("Number (blank to cancel): ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let n: usize = answer
        .trim()
        .parse()
        .context("Cancelled or invalid project selection")?;
    ensure!(n > 0 && n <= candidates.len(), "Invalid selection");
    Ok(n - 1)
}
fn warning(cli: &Cli) {
    if !cli.json {
        eprintln!(
            "Warning: this server may currently be running. Stop it before changing plugins. Plugin changes normally require a restart."
        );
    }
}
fn read_lock(root: &Path) -> Result<Lock> {
    if root.join(".plugget").exists() {
        state::safe_directory(&root.join(".plugget"))?;
    }
    ensure!(
        !root.join(".plugget/transaction.json").exists(),
        "Interrupted transaction detected. Run a mutating command to recover, or plugget doctor to inspect"
    );
    Lock::load(root)
}

pub async fn run(cli: &Cli, root: &Path) -> Result<Report> {
    let provider = Modrinth::new(cli.verbose && !cli.json)?;
    run_with(cli, root, &provider).await
}
pub async fn run_with(cli: &Cli, root: &Path, provider: &impl PackageProvider) -> Result<Report> {
    match &cli.command {
        Command::Version => Ok(report(
            json!({"name":"Plugget","version":env!("CARGO_PKG_VERSION")}),
            format!("Plugget {}", env!("CARGO_PKG_VERSION")),
        )),
        Command::Search { query, limit } => {
            let projects = provider.search(query, *limit).await?;
            let mut text =
                "NAME                     SLUG                     DESCRIPTION".to_string();
            for p in &projects {
                text.push_str(&format!(
                    "\n{:<24} {:<24} {}",
                    clean(&p.name),
                    clean(&p.slug),
                    clean(&p.description)
                ));
            }
            Ok(report(
                json!({"source":provider.name(),"results":projects}),
                text,
            ))
        }
        Command::Init {
            minecraft,
            platform,
        } => {
            let detection = minecraft::detect(root)?;
            ensure!(
                detection.server_properties
                    || !detection.jars.is_empty()
                    || detection.plugins_directory,
                "No Minecraft server detected in this directory"
            );
            let _guard = Guard::acquire(root)?;
            state::recover(root)?;
            let mut config = Config::load(root)?;
            config.minecraft = minecraft.clone().or(config.minecraft);
            config.platform = platform.or(config.platform);
            if interactive(cli) && (config.platform.is_none() || config.minecraft.is_none()) {
                if config.platform.is_none() {
                    eprint!("Platform (paper/purpur/spigot/bukkit): ");
                    io::stderr().flush()?;
                    let mut s = String::new();
                    io::stdin().read_line(&mut s)?;
                    config.platform = Some(
                        clap::ValueEnum::from_str(s.trim(), true)
                            .map_err(|e| anyhow::anyhow!(e))?,
                    );
                }
                if config.minecraft.is_none() {
                    eprint!("Minecraft version: ");
                    io::stderr().flush()?;
                    let mut s = String::new();
                    io::stdin().read_line(&mut s)?;
                    config.minecraft = Some(s.trim().into());
                }
            }
            config.server()?;
            let path = root.join(".plugget/config.toml");
            if path.exists() && (minecraft.is_some() || platform.is_some()) {
                confirm(cli, "Replace Plugget server configuration?")?;
            }
            if !path.exists() || minecraft.is_some() || platform.is_some() {
                state::atomic_write(&path, toml::to_string_pretty(&config)?.as_bytes())?;
            }
            if !root.join("plugins").exists() {
                fs::create_dir(root.join("plugins"))?;
            }
            state::safe_directory(&root.join("plugins"))?;
            let lock = Lock::load(root)?;
            if !root.join(".plugget/lock.json").exists() {
                state::atomic_write(
                    &root.join(".plugget/lock.json"),
                    &serde_json::to_vec_pretty(&lock)?,
                )?;
            }
            let found = state::jars(root)?.len();
            Ok(report(
                json!({"config":config,"existing_jars":found,"managed":lock.packages.len(),"warnings":detection.warnings}),
                format!(
                    "Plugget initialized.\nPlatform: {:?}\nMinecraft: {}\nPlugins directory: ./plugins\nFound {found} plugin jars; {} managed by Plugget.",
                    config.platform.unwrap(),
                    config.minecraft.unwrap(),
                    lock.packages.len()
                ),
            ))
        }
        Command::List => {
            state::ensure_initialized(root)?;
            let _guard = Guard::acquire(root)?;
            let lock = read_lock(root)?;
            let unmanaged: Vec<_> = state::jars(root)?
                .into_iter()
                .filter(|f| !lock.packages.values().any(|p| &p.filename == f))
                .collect();
            let mut rows = vec![];
            let mut text =
                "NAME                     VERSION          SOURCE       STATUS".to_string();
            for p in lock.packages.values() {
                let status = if !root.join("plugins").join(&p.filename).exists() {
                    "missing"
                } else {
                    "installed (run outdated to check updates)"
                };
                text.push_str(&format!(
                    "\n{:<24} {:<16} {:<12} {status}",
                    clean(&p.name),
                    clean(&p.version_number),
                    p.source
                ));
                rows.push(json!({"project_id":p.project_id,"name":p.name,"slug":p.slug,"version":p.version_number,"source":p.source,"filename":p.filename,"status":status}));
            }
            if !unmanaged.is_empty() {
                text.push_str("\n\nUnmanaged plugins:");
                for f in &unmanaged {
                    text.push_str(&format!("\n{}", clean(f)));
                }
            }
            Ok(report(json!({"packages":rows,"unmanaged":unmanaged}), text))
        }
        Command::Info { plugin } => {
            let config = Config::load(root)?;
            config.server()?;
            let project = packages::resolve(provider, plugin, &|p| choose(cli, p)).await?;
            let versions = provider.versions(&project.id).await?;
            let latest = packages::select(&versions, &config, None).ok();
            let lock = read_lock(root)?;
            let installed = lock.packages.get(&project.id);
            let update = latest.zip(installed).is_some_and(|(v, p)| {
                v.id != p.version_id
                    && v.published
                        > time::OffsetDateTime::parse(
                            &p.published,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            });
            let text = format!(
                "{} ({})\n{}\nSource: {}\nURL: {}\nAuthors: {}\nLatest compatible: {}\nMinecraft: {}\nLoaders: {}\nInstalled: {}\nUpdate available: {}\nDependencies: {}",
                clean(&project.name),
                clean(&project.slug),
                clean(&project.description),
                provider.name(),
                project.url,
                project.authors.join(", "),
                latest
                    .map(|v| clean(&v.number))
                    .unwrap_or_else(|| "none".into()),
                latest.map(|v| v.minecraft.join(", ")).unwrap_or_default(),
                latest.map(|v| v.loaders.join(", ")).unwrap_or_default(),
                installed
                    .map(|p| clean(&p.version_number))
                    .unwrap_or_else(|| "no".into()),
                update,
                latest
                    .map(|v| v
                        .dependencies
                        .iter()
                        .map(|d| format!(
                            "{}:{}",
                            d.kind,
                            d.project_id
                                .as_deref()
                                .or(d.version_id.as_deref())
                                .unwrap_or("external")
                        ))
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_default()
            );
            Ok(report(
                json!({"project":project,"source":provider.name(),"latest_compatible":latest.map(|v| json!({"id":v.id,"version":v.number,"channel":v.channel,"published":v.published.format(&time::format_description::well_known::Rfc3339).ok(),"minecraft":v.minecraft,"loaders":v.loaders,"dependencies":v.dependencies})),"installed_version":installed.map(|p| &p.version_number),"update_available":update}),
                clean_multiline(&text),
            ))
        }
        Command::Install {
            plugin,
            version,
            prerelease,
        } => {
            // A confidently detected server can be used directly without a separate init.
            let detected_config = if !root.join(".plugget/config.toml").exists() {
                let detected = minecraft::detect(root)?;
                ensure!(
                    detected.server_properties
                        || !detected.jars.is_empty()
                        || detected.plugins_directory,
                    "No Minecraft server detected. Enter a server directory and run plugget init"
                );
                let config = Config::load(root)?;
                config.server()?;
                Some(config)
            } else {
                None
            };
            let _guard = Guard::acquire(root)?;
            state::recover(root)?;
            if let Some(config) = detected_config {
                if !root.join("plugins").exists() {
                    fs::create_dir(root.join("plugins"))?;
                }
                if !root.join(".plugget/config.toml").exists() {
                    state::atomic_write(
                        &root.join(".plugget/config.toml"),
                        toml::to_string_pretty(&config)?.as_bytes(),
                    )?;
                }
            }
            state::ensure_initialized(root)?;
            let lock = Lock::load(root)?;
            let mut config = Config::load(root)?;
            if *prerelease {
                config.allow_prerelease = Some(true);
            }
            let project = packages::resolve(provider, plugin, &|p| choose(cli, p)).await?;
            let plan =
                packages::plan(provider, project, version.as_deref(), &config, &lock).await?;
            let summary = plan
                .iter()
                .filter(|p| {
                    lock.packages
                        .get(&p.project.id)
                        .is_none_or(|old| old.version_id != p.version.id)
                })
                .map(|p| format!("{} {}", p.project.slug, p.version.number))
                .collect::<Vec<_>>();
            if !summary.is_empty() {
                warning(cli);
                confirm(
                    cli,
                    &format!(
                        "Install {} (including required dependencies)?",
                        summary.join(", ")
                    ),
                )?;
            }
            let installed = packages::install_plan(provider, root, &lock, &plan).await?;
            Ok(report(
                json!({"installed":installed,"restart_required":!installed.is_empty()}),
                if installed.is_empty() {
                    "Already installed.".into()
                } else {
                    format!(
                        "Installed {}.\nRestart the server to apply changes.",
                        clean(&installed.join(", "))
                    )
                },
            ))
        }
        Command::Remove { plugin } => {
            state::ensure_initialized(root)?;
            let _guard = Guard::acquire(root)?;
            state::recover(root)?;
            let lock = Lock::load(root)?;
            let p = lock.find(plugin)?;
            warning(cli);
            confirm(
                cli,
                &format!(
                    "Move {} ({}) to the OS Recycle Bin? Plugin data will be retained",
                    p.name, p.filename
                ),
            )?;
            let removed = packages::remove(root, &lock, plugin)?;
            Ok(report(
                json!({"removed":removed,"restart_required":true}),
                format!(
                    "Removed {}. Plugin data retained.\nRestart the server to apply changes.",
                    clean(&removed)
                ),
            ))
        }
        Command::Outdated => check_updates(cli, root, provider, None, false, false).await,
        Command::Update {
            plugin, prerelease, ..
        } => check_updates(cli, root, provider, plugin.as_deref(), true, *prerelease).await,
        Command::Doctor { offline } => doctor(root, provider, *offline).await,
    }
}
fn clean_multiline(s: &str) -> String {
    s.lines().map(clean).collect::<Vec<_>>().join("\n")
}

async fn check_updates(
    cli: &Cli,
    root: &Path,
    provider: &impl PackageProvider,
    query: Option<&str>,
    apply: bool,
    prerelease: bool,
) -> Result<Report> {
    state::ensure_initialized(root)?;
    let _guard = Guard::acquire(root)?;
    if apply {
        state::recover(root)?;
    }
    let mut lock = read_lock(root)?;
    let mut config = Config::load(root)?;
    if prerelease {
        config.allow_prerelease = Some(true);
    }
    config.server()?;
    let ids: Vec<_> = if let Some(q) = query {
        vec![lock.find(q)?.project_id.clone()]
    } else {
        lock.packages.keys().cloned().collect()
    };
    let mut updates = vec![];
    let mut errors = vec![];
    let mut text = String::new();
    for id in ids {
        let old = lock.packages[&id].clone();
        let result = async {
            let versions = provider.versions(&id).await?;
            let latest = packages::select(&versions, &config, None)?;
            ensure!(latest.project_id == id, "Version project identity mismatch");
            let published = time::OffsetDateTime::parse(&old.published, &time::format_description::well_known::Rfc3339)?;
            if latest.id == old.version_id || latest.published <= published { return Ok::<_,anyhow::Error>(None); }
            if apply {
                let project = provider.project(&id).await?.context("Managed project no longer exists")?;
                let plan = packages::plan(provider, project, Some(&latest.id), &config, &lock).await?;
                let summary = plan.iter().map(|p| format!("{} {}",p.project.slug,p.version.number)).collect::<Vec<_>>().join(", ");
                warning(cli); confirm(cli, &format!("Update {} {} -> {}? Plan: {summary}",old.name,old.version_number,latest.number))?;
                packages::install_plan(provider, root, &lock, &plan).await?;
            }
            Ok(Some(json!({"project_id":id,"name":old.name,"installed":old.version_number,"latest":latest.number,"compatible":true,"updated":apply})))
        }.await;
        match result {
            Ok(Some(row)) => {
                text.push_str(&format!(
                    "{}  {} -> {}{}\n",
                    clean(&old.name),
                    clean(&old.version_number),
                    row["latest"].as_str().map(clean).unwrap_or_default(),
                    if apply { " (updated)" } else { "" }
                ));
                updates.push(row);
            }
            Ok(None) => {}
            Err(e) => {
                errors.push(json!({"project_id":id,"message":format!("{e:#}")}));
                text.push_str(&format!(
                    "Failed {}: {}\n",
                    clean(&old.name),
                    clean(&format!("{e:#}"))
                ));
            }
        }
        if apply {
            lock = Lock::load(root)?;
            if root.join(".plugget/transaction.json").exists() {
                break;
            }
        }
    }
    text.push_str(&format!(
        "{} {}; {} failures.",
        updates.len(),
        if apply {
            "updates applied"
        } else {
            "updates available"
        },
        errors.len()
    ));
    Ok(Report {
        code: if errors.is_empty() { 0 } else { 3 },
        data: json!({"updates":updates,"errors":errors,"restart_required":apply && !updates.is_empty()}),
        text,
    })
}

async fn doctor(root: &Path, provider: &impl PackageProvider, offline: bool) -> Result<Report> {
    let mut issues = vec![];
    let detection = minecraft::detect(root)?;
    if !detection.server_properties && detection.jars.is_empty() {
        issues.push("No server.properties or server JAR detected".to_string());
    }
    if !detection.plugins_directory {
        issues.push("No plugins directory".into());
    }
    match Config::load(root).and_then(|c| c.server().map(|_| ())) {
        Ok(_) => {}
        Err(e) => issues.push(format!("Server configuration: {e:#}")),
    }
    if !root.join(".plugget/config.toml").is_file() {
        issues.push("Plugget is not initialized".into());
    }
    if root.join(".plugget/transaction.json").exists() {
        issues.push("Interrupted transaction or pending Recycle Bin cleanup; a mutating command will attempt recovery".into());
    }
    match Lock::load(root) {
        Ok(lock) => {
            for p in lock.packages.values() {
                if let Err(e) = state::verify_owned(root, p) {
                    issues.push(format!("{}: {e:#}", p.slug));
                }
            }
        }
        Err(e) => issues.push(format!("Invalid metadata: {e:#}")),
    }
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut hashes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    match state::jars(root) {
        Ok(jars) => {
            for name in jars {
                let path = root.join("plugins").join(&name);
                match state::hash_file(&path) {
                    Ok(hash) => {
                        hashes.entry(hash).or_default().push(name.clone());
                        if let Some(plugin) = minecraft::plugin_name(&path) {
                            names.entry(plugin).or_default().push(name);
                        }
                    }
                    Err(e) => issues.push(format!("Unsafe plugin file: {e:#}")),
                }
            }
        }
        Err(e) => issues.push(format!("Plugins directory: {e:#}")),
    }
    for files in names
        .values()
        .chain(hashes.values())
        .filter(|files| files.len() > 1)
    {
        issues.push(format!("Duplicate plugin jars: {}", files.join(", ")));
    }
    if root.join(".plugget").exists() {
        state::safe_directory(&root.join(".plugget"))?;
        for entry in fs::read_dir(root.join(".plugget"))? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if name.starts_with("transaction-") || name.starts_with("write-") {
                issues.push(format!("Retained staging/temporary files: .plugget/{name}"));
            }
        }
    }
    if !offline && let Err(e) = provider.search("luckperms", 1).await {
        issues.push(format!("Network connectivity: {e:#}"));
    }
    issues.extend(detection.warnings);
    issues.sort();
    issues.dedup();
    let text = if issues.is_empty() {
        "No problems found. Plugin changes require a server restart.".into()
    } else {
        issues
            .iter()
            .map(|s| format!("! {}", clean(s)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(Report {
        code: if issues.is_empty() { 0 } else { 4 },
        data: json!({"issues":issues,"network_checked":!offline}),
        text,
    })
}
