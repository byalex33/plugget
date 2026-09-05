use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plugget"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}
#[test]
fn executable_init_list_doctor_remove_and_json_errors() {
    let root = tempfile::Builder::new()
        .prefix("plugget-cli-test-")
        .tempdir()
        .unwrap()
        .keep();
    fs::write(root.join("server.properties"), "").unwrap();
    fs::create_dir(root.join("plugins")).unwrap();
    fs::write(root.join("plugins/private.jar"), b"private").unwrap();
    let output = run(
        &root,
        &[
            "init",
            "--platform",
            "paper",
            "--minecraft",
            "1.21.11",
            "--json",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["existing_jars"], 1);
    let output = run(&root, &["list", "--json"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["unmanaged"][0], "private.jar");
    let output = run(&root, &["remove", "private", "--yes", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(root.join("plugins/private.jar")).unwrap(),
        b"private"
    );
    let output = run(&root, &["doctor", "--offline", "--json"]);
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    let output = run(&root, &["search", "test", "--limit", "101", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    let output = run(&root, &["version", "--quiet"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    for command in [
        "init", "search", "info", "install", "remove", "list", "outdated", "update", "doctor",
        "version",
    ] {
        assert!(run(&root, &[command, "--help"]).status.success());
    }
}
