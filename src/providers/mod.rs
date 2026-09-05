pub mod modrinth;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub filename: String,
    pub url: String,
    pub sha512: String,
    pub size: u64,
    pub primary: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub number: String,
    pub published: time::OffsetDateTime,
    pub channel: String,
    pub minecraft: Vec<String>,
    pub loaders: Vec<String>,
    pub dependencies: Vec<Dependency>,
    pub artifacts: Vec<Artifact>,
}

/// Provider-neutral data keeps resolution and filesystem transactions independent of the API.
pub trait PackageProvider {
    fn name(&self) -> &'static str;
    fn search(&self, query: &str, limit: u8) -> impl Future<Output = Result<Vec<Project>>>;
    fn project(&self, id: &str) -> impl Future<Output = Result<Option<Project>>>;
    fn versions(&self, id: &str) -> impl Future<Output = Result<Vec<Version>>>;
    fn version(&self, id: &str) -> impl Future<Output = Result<Version>>;
    fn download(
        &self,
        artifact: &Artifact,
        path: &std::path::Path,
    ) -> impl Future<Output = Result<()>>;
}
