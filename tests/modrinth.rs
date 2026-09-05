use clap::Parser;
use plugget::{
    cli::Cli,
    commands,
    providers::{PackageProvider, modrinth::Modrinth},
    state::{self, Lock},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header_exists, method, path, query_param},
};

fn jar() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    zip.start_file("plugin.yml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"name: TestPlugin\nversion: 1\n").unwrap();
    zip.finish().unwrap().into_inner()
}
fn version(base: &str, id: &str, mc: &str, day: u8, bytes: &[u8]) -> Value {
    json!({"id":id,"project_id":"project","version_number":id,"date_published":format!("2026-01-{day:02}T00:00:00Z"),"version_type":"release","game_versions":[mc],"loaders":["bukkit"],"dependencies":[],"files":[{"filename":format!("plugin-{id}.jar"),"url":format!("{base}/download/{id}"),"hashes":{"sha512":format!("{:x}",Sha512::digest(bytes))},"size":bytes.len(),"primary":true}]})
}
async fn setup() -> (MockServer, Modrinth, PathBuf) {
    let mock = MockServer::start().await;
    let provider = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    let root = tempfile::Builder::new()
        .prefix("plugget-http-test-")
        .tempdir()
        .unwrap()
        .keep();
    fs::write(root.join("server.properties"), b"").unwrap();
    let cli = Cli::parse_from([
        "plugget",
        "init",
        "--platform",
        "paper",
        "--minecraft",
        "1.21.11",
        "--json",
    ]);
    commands::run_with(&cli, &root, &provider).await.unwrap();
    for id in ["test", "project"] {
        Mock::given(method("GET")).and(path(format!("/v2/project/{id}"))).and(header_exists("user-agent")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"project","slug":"test","title":"TestPlugin","description":"Test plugin"}))).mount(&mock).await;
    }
    Mock::given(path("/v2/project/project/members"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{ "user":{"username":"author"}}])),
        )
        .mount(&mock)
        .await;
    (mock, provider, root)
}
#[tokio::test]
async fn fresh_install_selects_older_compatible_release_and_writes_lock() {
    let (mock, provider, root) = setup().await;
    let bytes = jar();
    Mock::given(path("/v2/project/project/version"))
        .and(query_param("include_changelog", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            version(&mock.uri(), "new-incompatible", "1.22", 3, &bytes),
            version(&mock.uri(), "v1", "1.21.11", 1, &bytes)
        ])))
        .mount(&mock)
        .await;
    Mock::given(path("/download/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .expect(1)
        .mount(&mock)
        .await;
    let result = commands::run_with(
        &Cli::parse_from(["plugget", "install", "test", "--yes", "--json"]),
        &root,
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(result.code, 0);
    let lock = Lock::load(&root).unwrap();
    assert_eq!(lock.packages["project"].version_id, "v1");
    assert_eq!(fs::read(root.join("plugins/plugin-v1.jar")).unwrap(), bytes);
    assert!(!root.join(".plugget/transaction.json").exists());
}
#[tokio::test]
async fn failed_update_hash_preserves_existing_jar_and_lock() {
    let (mock, provider, root) = setup().await;
    let bytes = jar();
    Mock::given(path("/v2/project/project/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &mock.uri(),
            "v1",
            "1.21.11",
            1,
            &bytes
        )])))
        .mount(&mock)
        .await;
    Mock::given(path("/download/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .mount(&mock)
        .await;
    commands::run_with(
        &Cli::parse_from(["plugget", "install", "test", "--yes"]),
        &root,
        &provider,
    )
    .await
    .unwrap();
    let before = Lock::load(&root).unwrap();
    mock.reset().await;
    Mock::given(path("/v2/project/project"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"id":"project","slug":"test","title":"TestPlugin","description":"Test"}),
        ))
        .mount(&mock)
        .await;
    Mock::given(path("/v2/project/project/members"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&mock)
        .await;
    Mock::given(path("/v2/project/project/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &mock.uri(),
            "v2",
            "1.21.11",
            2,
            &bytes
        )])))
        .mount(&mock)
        .await;
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    Mock::given(path("/download/v2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(corrupt))
        .mount(&mock)
        .await;
    let fresh = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    let report = commands::run_with(
        &Cli::parse_from(["plugget", "update", "--all", "--yes", "--json"]),
        &root,
        &fresh,
    )
    .await
    .unwrap();
    assert_eq!(report.code, 3);
    assert!(report.text.contains("checksum"));
    assert_eq!(Lock::load(&root).unwrap(), before);
    assert_eq!(fs::read(root.join("plugins/plugin-v1.jar")).unwrap(), bytes);
    assert!(!root.join("plugins/plugin-v2.jar").exists());
}
#[tokio::test]
async fn malformed_api_data_and_redirects_are_errors() {
    let mock = MockServer::start().await;
    let provider = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    Mock::given(path("/v2/project/bad/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id":"missing-fields"}])))
        .mount(&mock)
        .await;
    assert!(provider.versions("bad").await.is_err());
    Mock::given(path("/v2/project/redirect/version"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "https://example.com"))
        .mount(&mock)
        .await;
    assert!(
        provider
            .versions("redirect")
            .await
            .unwrap_err()
            .to_string()
            .contains("redirect")
    );
}
#[tokio::test]
async fn ambiguity_never_silently_installs_even_with_yes() {
    let (mock, provider, root) = setup().await;
    Mock::given(path("/v2/project/ambiguous"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;
    Mock::given(path("/v2/search")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"hits":[{"project_id":"a","slug":"one","title":"One","description":"","author":"a"},{"project_id":"b","slug":"two","title":"Two","description":"","author":"b"}]}))).mount(&mock).await;
    let error = commands::run_with(
        &Cli::parse_from(["plugget", "install", "ambiguous", "--yes", "--json"]),
        &root,
        &provider,
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("one"));
    assert!(error.to_string().contains("two"));
    assert!(Lock::load(&root).unwrap().packages.is_empty());
}
#[tokio::test]
async fn rate_limits_are_bounded_and_metadata_is_cached() {
    let mock = MockServer::start().await;
    let provider = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    Mock::given(path("/v2/search"))
        .and(query_param("query", "limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .expect(1)
        .mount(&mock)
        .await;
    assert!(
        provider
            .search("limited", 1)
            .await
            .unwrap_err()
            .to_string()
            .contains("rate limited")
    );
    Mock::given(path("/v2/search"))
        .and(query_param("query", "cached"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"hits":[]})))
        .expect(1)
        .mount(&mock)
        .await;
    provider.search("cached", 1).await.unwrap();
    provider.search("cached", 1).await.unwrap();
}
#[tokio::test]
async fn unmanaged_collision_and_doctor_duplicate_detection() {
    let (mock, provider, root) = setup().await;
    let bytes = jar();
    fs::write(root.join("plugins/plugin-v1.jar"), &bytes).unwrap();
    fs::write(root.join("plugins/private.jar"), &bytes).unwrap();
    Mock::given(path("/v2/project/project/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &mock.uri(),
            "v1",
            "1.21.11",
            1,
            &bytes
        )])))
        .mount(&mock)
        .await;
    Mock::given(path("/download/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .mount(&mock)
        .await;
    assert!(
        commands::run_with(
            &Cli::parse_from(["plugget", "install", "test", "--yes"]),
            &root,
            &provider
        )
        .await
        .is_err()
    );
    assert!(Lock::load(&root).unwrap().packages.is_empty());
    let report = commands::run_with(
        &Cli::parse_from(["plugget", "doctor", "--offline", "--json"]),
        &root,
        &provider,
    )
    .await
    .unwrap();
    assert_eq!(report.code, 4);
    assert!(report.text.contains("Duplicate plugin jars"));
    assert_eq!(
        state::hash_file(&root.join("plugins/private.jar")).unwrap(),
        format!("{:x}", Sha512::digest(bytes))
    );
}

#[tokio::test]
async fn invalid_download_urls_hashes_and_corrupt_zip_are_rejected() {
    use plugget::providers::Artifact;
    let mock = MockServer::start().await;
    let provider = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    let root = tempfile::tempdir().unwrap().keep();
    let mut a = Artifact {
        filename: "a.jar".into(),
        url: "http://evil.example/a.jar".into(),
        sha512: "a".repeat(128),
        size: 10,
        primary: true,
    };
    assert!(provider.download(&a, &root.join("http.jar")).await.is_err());
    assert!(!root.join("http.jar").exists());
    a.url = "https://evil.example/a.jar".into();
    assert!(provider.download(&a, &root.join("host.jar")).await.is_err());
    a.url = format!("{}/jar", mock.uri());
    a.sha512 = "invalid".into();
    assert!(provider.download(&a, &root.join("hash.jar")).await.is_err());
    let bytes = b"not a jar!";
    a.sha512 = format!("{:x}", Sha512::digest(bytes));
    a.size = bytes.len() as u64;
    Mock::given(path("/jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
        .mount(&mock)
        .await;
    assert!(
        provider
            .download(&a, &root.join("invalid.jar"))
            .await
            .unwrap_err()
            .to_string()
            .contains("JAR")
    );
}

#[tokio::test]
async fn install_auto_initializes_confidently_detected_server() {
    let (mock, provider, _) = setup().await;
    let root = tempfile::tempdir().unwrap().keep();
    fs::write(root.join("paper-1.21.11-1.jar"), b"test fixture").unwrap();
    let bytes = jar();
    Mock::given(path("/v2/project/project/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &mock.uri(),
            "v1",
            "1.21.11",
            1,
            &bytes
        )])))
        .mount(&mock)
        .await;
    Mock::given(path("/download/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&mock)
        .await;
    commands::run_with(
        &Cli::parse_from(["plugget", "install", "test", "--yes", "--json"]),
        &root,
        &provider,
    )
    .await
    .unwrap();
    assert!(root.join(".plugget/config.toml").exists());
    assert_eq!(Lock::load(&root).unwrap().packages.len(), 1);
}

#[tokio::test]
async fn safe_get_retries_transient_server_error() {
    let mock = MockServer::start().await;
    let provider = Modrinth::with_base(&format!("{}/v2/", mock.uri()), true, false).unwrap();
    Mock::given(path("/v2/search"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/v2/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"hits":[]})))
        .with_priority(2)
        .expect(1)
        .mount(&mock)
        .await;
    provider.search("retry", 1).await.unwrap();
}
