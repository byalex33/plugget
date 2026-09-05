use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Paper,
    Purpur,
    Spigot,
    Bukkit,
}

impl Platform {
    pub fn loaders(self) -> &'static [&'static str] {
        match self {
            Self::Purpur => &["purpur", "paper", "spigot", "bukkit"],
            Self::Paper => &["paper", "spigot", "bukkit"],
            Self::Spigot => &["spigot", "bukkit"],
            Self::Bukkit => &["bukkit"],
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Detection {
    pub server_properties: bool,
    pub plugins_directory: bool,
    pub jars: Vec<String>,
    pub platform: Option<Platform>,
    pub minecraft: Option<String>,
    pub warnings: Vec<String>,
}

pub fn valid_minecraft(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|c| c.is_ascii_digit()))
}

fn named_version(name: &str) -> Option<String> {
    name.trim_end_matches(".jar")
        .split(['-', '_'])
        .find(|s| valid_minecraft(s))
        .map(str::to_owned)
}

pub fn jar_text(path: &Path, entry: &str) -> Result<String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    let file = archive.by_name(entry)?;
    if file.size() > 128 * 1024 {
        bail!("Jar metadata too large");
    }
    let mut text = String::new();
    file.take(128 * 1024 + 1).read_to_string(&mut text)?;
    Ok(text)
}

pub fn detect(root: &Path) -> Result<Detection> {
    let mut result = Detection {
        server_properties: root.join("server.properties").is_file(),
        plugins_directory: root.join("plugins").is_dir(),
        ..Default::default()
    };
    let mut platforms = BTreeSet::new();
    let mut versions = BTreeSet::new();
    for entry in fs::read_dir(root).context("Could not inspect server directory")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type()?.is_file() || !name.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        result.jars.push(name.clone());
        let lower = name.to_ascii_lowercase();
        let platform = if lower.starts_with("purpur") {
            Some(Platform::Purpur)
        } else if lower.starts_with("paper") {
            Some(Platform::Paper)
        } else if lower.starts_with("spigot") {
            Some(Platform::Spigot)
        } else if lower.starts_with("craftbukkit") {
            Some(Platform::Bukkit)
        } else {
            None
        };
        if let Some(p) = platform {
            platforms.insert(p);
            if let Some(v) = named_version(&lower) {
                versions.insert(v);
            }
        }
        if let Ok(text) = jar_text(&entry.path(), "version.json")
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(id) = value["id"].as_str().filter(|s| valid_minecraft(s))
        {
            versions.insert(id.to_owned());
        }
        if let Ok(text) = jar_text(&entry.path(), "META-INF/MANIFEST.MF") {
            let lower = text.to_ascii_lowercase();
            if lower.contains("purpur") {
                platforms.insert(Platform::Purpur);
            } else if lower.contains("papermc") || lower.contains("paperclip") {
                platforms.insert(Platform::Paper);
            } else if lower.contains("spigot") {
                platforms.insert(Platform::Spigot);
            } else if lower.contains("craftbukkit") {
                platforms.insert(Platform::Bukkit);
            }
        }
    }
    // Only current structural evidence is used; old logs can describe a previous server version.
    if platforms.is_empty() {
        if root.join("config/paper-global.yml").is_file() || root.join("paper.yml").is_file() {
            platforms.insert(Platform::Paper);
        } else if root.join("spigot.yml").is_file() {
            platforms.insert(Platform::Spigot);
        } else if root.join("bukkit.yml").is_file() {
            platforms.insert(Platform::Bukkit);
        }
    }
    if platforms.len() == 1 {
        result.platform = platforms.first().copied();
    } else if platforms.len() > 1 {
        result
            .warnings
            .push("Conflicting server platforms; set an explicit --platform override.".into());
    }
    if versions.len() == 1 {
        result.minecraft = versions.first().cloned();
    } else if versions.len() > 1 {
        result
            .warnings
            .push("Conflicting Minecraft versions; set an explicit --minecraft override.".into());
    }
    result.jars.sort();
    Ok(result)
}

pub fn plugin_name(path: &Path) -> Option<String> {
    ["paper-plugin.yml", "plugin.yml"].iter().find_map(|entry| {
        jar_text(path, entry).ok()?.lines().find_map(|line| {
            line.strip_prefix("name:")
                .map(|v| v.trim().trim_matches(['\'', '"']).to_ascii_lowercase())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_is_directional() {
        assert!(Platform::Paper.loaders().contains(&"bukkit"));
        assert!(Platform::Purpur.loaders().contains(&"paper"));
        assert!(!Platform::Spigot.loaders().contains(&"paper"));
        assert!(!Platform::Bukkit.loaders().contains(&"spigot"));
        assert!(!Platform::Paper.loaders().contains(&"folia"));
    }
    #[test]
    fn exact_versions() {
        for v in ["1.21.11", "1.8", "26.1"] {
            assert!(valid_minecraft(v));
        }
        for v in ["", "1", "1.21.x", "1.21.1.2", "../1.2"] {
            assert!(!valid_minecraft(v));
        }
        assert_eq!(
            named_version("paper-1.21.11-42.jar"),
            Some("1.21.11".into())
        );
    }
}
