use super::{Artifact, Dependency, PackageProvider, Project, Version};
use crate::network::Http;
use anyhow::{Context, Result, ensure};
use reqwest::{StatusCode, Url};
use serde::Deserialize;

pub struct Modrinth {
    http: Http,
    base: Url,
}
impl Modrinth {
    pub fn new(verbose: bool) -> Result<Self> {
        Self::with_base("https://api.modrinth.com/v2/", false, verbose)
    }
    /// Explicit injection for isolated integration tests; the CLI has no endpoint override.
    pub fn with_base(base: &str, local_test: bool, verbose: bool) -> Result<Self> {
        Ok(Self {
            http: Http::new(local_test, verbose)?,
            base: Url::parse(base)?,
        })
    }
    fn endpoint(&self, segments: &[&str]) -> Result<Url> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid API base"))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }
}

#[derive(Deserialize)]
struct RawProject {
    id: String,
    slug: String,
    title: String,
    description: String,
}
impl From<RawProject> for Project {
    fn from(p: RawProject) -> Self {
        Self {
            url: format!("https://modrinth.com/plugin/{}", p.slug),
            id: p.id,
            slug: p.slug,
            name: p.title,
            description: p.description,
            authors: vec![],
        }
    }
}
#[derive(Deserialize)]
struct Hit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
}
#[derive(Deserialize)]
struct Search {
    hits: Vec<Hit>,
}
#[derive(Deserialize)]
struct Member {
    user: User,
}
#[derive(Deserialize)]
struct User {
    username: String,
}
#[derive(Deserialize)]
struct RawDependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}
#[derive(Deserialize)]
struct File {
    filename: String,
    url: String,
    hashes: Hashes,
    size: u64,
    primary: bool,
    file_type: Option<String>,
}
#[derive(Deserialize)]
struct Hashes {
    sha512: String,
}
#[derive(Deserialize)]
struct RawVersion {
    id: String,
    project_id: String,
    version_number: String,
    date_published: String,
    version_type: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    dependencies: Vec<RawDependency>,
    files: Vec<File>,
}
impl TryFrom<RawVersion> for Version {
    type Error = anyhow::Error;
    fn try_from(v: RawVersion) -> Result<Self> {
        ensure!(
            !v.id.is_empty() && !v.project_id.is_empty() && !v.version_number.is_empty(),
            "Missing version identity"
        );
        ensure!(
            ["release", "beta", "alpha"].contains(&v.version_type.as_str()),
            "Unknown release channel"
        );
        Ok(Self {
            id: v.id,
            project_id: v.project_id,
            number: v.version_number,
            published: time::OffsetDateTime::parse(
                &v.date_published,
                &time::format_description::well_known::Rfc3339,
            )
            .context("Invalid publication timestamp")?,
            channel: v.version_type,
            minecraft: v.game_versions,
            loaders: v.loaders,
            dependencies: v
                .dependencies
                .into_iter()
                .map(|d| Dependency {
                    project_id: d.project_id,
                    version_id: d.version_id,
                    kind: d.dependency_type,
                })
                .collect(),
            artifacts: v
                .files
                .into_iter()
                .filter(|f| {
                    f.file_type.as_deref().is_none_or(|t| t == "unknown")
                        && f.filename.to_ascii_lowercase().ends_with(".jar")
                })
                .map(|f| Artifact {
                    filename: f.filename,
                    url: f.url,
                    sha512: f.hashes.sha512,
                    size: f.size,
                    primary: f.primary,
                })
                .collect(),
        })
    }
}

impl PackageProvider for Modrinth {
    fn name(&self) -> &'static str {
        "modrinth"
    }
    async fn search(&self, query: &str, limit: u8) -> Result<Vec<Project>> {
        let mut url = self.endpoint(&["search"])?;
        url.query_pairs_mut().append_pair("query", query).append_pair("limit", &limit.to_string()).append_pair("facets", r#"[["categories:paper","categories:purpur","categories:spigot","categories:bukkit"]]"#);
        let result: Search = self.http.json(url).await?;
        Ok(result
            .hits
            .into_iter()
            .map(|h| Project {
                id: h.project_id,
                url: format!("https://modrinth.com/plugin/{}", h.slug),
                slug: h.slug,
                name: h.title,
                description: h.description,
                authors: vec![h.author],
            })
            .collect())
    }
    async fn project(&self, id: &str) -> Result<Option<Project>> {
        match self
            .http
            .json::<RawProject>(self.endpoint(&["project", id])?)
            .await
        {
            Ok(raw) => {
                let mut project: Project = raw.into();
                let members: Vec<Member> = self
                    .http
                    .json(self.endpoint(&["project", &project.id, "members"])?)
                    .await?;
                project.authors = members.into_iter().map(|m| m.user.username).collect();
                Ok(Some(project))
            }
            Err(e)
                if e.downcast_ref::<reqwest::Error>().and_then(|e| e.status())
                    == Some(StatusCode::NOT_FOUND) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    async fn versions(&self, id: &str) -> Result<Vec<Version>> {
        let mut url = self.endpoint(&["project", id, "version"])?;
        url.query_pairs_mut()
            .append_pair("include_changelog", "false");
        self.http
            .json::<Vec<RawVersion>>(url)
            .await?
            .into_iter()
            .map(Version::try_from)
            .collect()
    }
    async fn version(&self, id: &str) -> Result<Version> {
        self.http
            .json::<RawVersion>(self.endpoint(&["version", id])?)
            .await?
            .try_into()
    }
    async fn download(&self, artifact: &Artifact, path: &std::path::Path) -> Result<()> {
        self.http.download(artifact, path).await
    }
}
