use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, StatusCode, Url};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct Http {
    client: Client,
    cache: Mutex<HashMap<String, (Instant, Vec<u8>)>>,
    local_test: bool,
    verbose: bool,
}

impl Http {
    pub fn new(local_test: bool, verbose: bool) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent(concat!(
                    "Plugget/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://plugget.dev)"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            cache: Mutex::new(HashMap::new()),
            local_test,
            verbose,
        })
    }

    fn check_url(&self, url: &Url, binary: bool) -> Result<()> {
        let local = self.local_test
            && url.scheme() == "http"
            && url
                .host_str()
                .is_some_and(|h| h == "127.0.0.1" || h == "[::1]");
        ensure!(
            local || url.scheme() == "https",
            "Only HTTPS URLs are accepted"
        );
        ensure!(
            url.username().is_empty() && url.password().is_none(),
            "URL credentials are forbidden"
        );
        if binary && !local {
            ensure!(
                url.host_str() == Some("cdn.modrinth.com")
                    && url.port_or_known_default() == Some(443),
                "Unexpected download host; expected cdn.modrinth.com"
            );
        }
        Ok(())
    }

    async fn get(&self, url: Url, binary: bool) -> Result<reqwest::Response> {
        self.check_url(&url, binary)?;
        for attempt in 0..3u64 {
            if self.verbose {
                eprintln!(
                    "GET {} {} (attempt {})",
                    url.host_str().unwrap_or(""),
                    url.path(),
                    attempt + 1
                );
            }
            match self.client.get(url.clone()).send().await {
                Ok(response) => {
                    let status = response.status();
                    if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                        && attempt < 2
                    {
                        let delay = response
                            .headers()
                            .get("retry-after")
                            .or_else(|| response.headers().get("x-ratelimit-reset"))
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(attempt + 1);
                        ensure!(
                            delay <= 30,
                            "Modrinth rate limited this request; retry in {delay} seconds"
                        );
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        continue;
                    }
                    ensure!(
                        !status.is_redirection(),
                        "Unexpected HTTP redirect from {}; request stopped",
                        url.host_str().unwrap_or("server")
                    );
                    return response
                        .error_for_status()
                        .context("Modrinth HTTP request failed");
                }
                Err(error) if attempt < 2 && (error.is_timeout() || error.is_connect()) => {
                    tokio::time::sleep(Duration::from_secs(attempt + 1)).await
                }
                Err(error) => {
                    return Err(error)
                        .context("Network request failed; check your connection and retry");
                }
            }
        }
        bail!("Request retries exhausted")
    }

    pub async fn json<T: DeserializeOwned>(&self, url: Url) -> Result<T> {
        let key = url.to_string();
        if let Some((created, bytes)) = self.cache.lock().expect("cache mutex").get(&key)
            && created.elapsed() < Duration::from_secs(30)
        {
            return serde_json::from_slice(bytes).context("Malformed cached metadata");
        }
        let mut response = self.get(url, false).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 32 * 1024 * 1024,
                "API response exceeds metadata limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        let value = serde_json::from_slice(&bytes).context("Malformed Modrinth response")?;
        self.cache
            .lock()
            .expect("cache mutex")
            .insert(key, (Instant::now(), bytes));
        Ok(value)
    }

    pub async fn download(&self, artifact: &crate::providers::Artifact, path: &Path) -> Result<()> {
        ensure!(
            artifact.size > 0 && artifact.size <= 512 * 1024 * 1024,
            "Invalid artifact size (limit: 512 MiB)"
        );
        crate::state::valid_hash(&artifact.sha512)?;
        let mut response = self.get(Url::parse(&artifact.url)?, true).await?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut hash = Sha512::new();
        let mut size = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Download interrupted; existing plugins remain unchanged")?
        {
            size += chunk.len() as u64;
            ensure!(size <= artifact.size, "Download exceeds declared size");
            file.write_all(&chunk)?;
            hash.update(&chunk);
        }
        ensure!(
            size == artifact.size,
            "Incomplete download: expected {} bytes, got {size}",
            artifact.size
        );
        ensure!(
            format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&artifact.sha512),
            "SHA512 checksum mismatch; existing plugins remain unchanged"
        );
        file.sync_all()?;
        zip::ZipArchive::new(std::fs::File::open(path)?)
            .context("Downloaded file is not a valid JAR/ZIP archive")?;
        Ok(())
    }
}
