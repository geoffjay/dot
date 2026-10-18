//! CLI integration tests: full command sequences against a temp HOME.
//!
//! The `example` module is embedded, so every test drives real module
//! loading, rendering, and the manifest without touching the developer's
//! $HOME.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn dot() -> Command {
    Command::cargo_bin("dot").expect("dot binary is built")
}

fn env_home(cmd: &mut Command, home: &TempDir) {
    // isolate everything: HOME drives config/state resolution
    cmd.env("HOME", home.path());
    cmd.env_remove("XDG_STATE_HOME");
    cmd.env_remove("DOT_CONFIG");
}

fn answers_file(home: &TempDir) -> std::path::PathBuf {
    let path = home.path().join("answers.toml");
    // `modules` must precede [user]: in TOML, keys after a table header
    // belong to that table.
    fs::write(
        &path,
        r#"
modules = ["example"]

[user]
name = "Tester"
email = "t@example.com"
theme = "gruvbox"
"#,
    )
    .unwrap();
    path
}

fn init(home: &TempDir) {
    let answers = answers_file(home);
    let mut cmd = dot();
    env_home(&mut cmd, home);
    cmd.args([
        "init",
        "--non-interactive",
        "--answers",
        answers.to_str().unwrap(),
    ])
    .assert()
    .success();
}

#[test]
fn init_writes_config() {
    let home = TempDir::new().unwrap();
    init(&home);

    let cfg = fs::read_to_string(home.path().join(".config/dot/config.toml")).unwrap();
    assert!(cfg.contains("Tester"));
    assert!(cfg.contains("gruvbox"));
    // module selection recorded as explicit enable entries
    assert!(cfg.contains("[modules.example]"));
    assert!(cfg.contains("enabled = true"));
}

#[test]
fn init_refuses_existing_without_force() {
    let home = TempDir::new().unwrap();
    init(&home);
    let answers = answers_file(&home);

    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args([
        "init",
        "--non-interactive",
        "--answers",
        answers.to_str().unwrap(),
    ])
    .assert()
    .failure()
    .stderr(contains("already exists"));

    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args([
        "init",
        "--force",
        "--non-interactive",
        "--answers",
        answers.to_str().unwrap(),
    ])
    .assert()
    .success();
}

#[test]
fn write_diff_backup_restore_propose_sequence() {
    let home = TempDir::new().unwrap();
    init(&home);

    // diff: everything is new -> exit 1 (differences)
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff"]);
    cmd.assert().code(1);

    // write: creates the example module's file
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["write"]);
    cmd.assert().success().stdout(contains("1 created"));

    let target = home.path().join(".dot-example.txt");
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("hello, Tester!"));
    assert!(content.contains("gruvbox"));

    // diff: identical now -> exit 0
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff"]);
    cmd.assert().code(0);

    // mutate the managed file on disk: diff reports it again
    fs::write(&target, "local edit\n").unwrap();
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff"]);
    cmd.assert().code(1);

    // backup captures the current (locally edited) state
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["backup"]);
    cmd.assert().success().stdout(contains("backed up 1 files"));

    // change the file again after the backup: restore refuses ...
    fs::write(&target, "changed after backup\n").unwrap();
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["restore"]);
    cmd.assert().failure().stderr(contains("--force"));

    // ... and restores the snapshot with --force
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["restore", "--force"]);
    cmd.assert()
        .success()
        .stdout(contains("1 file(s) restored"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "local edit\n");

    // restore --list shows the snapshot
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["restore", "--list"]);
    cmd.assert().success();

    // propose dry-run: the on-disk edit differs from rendered output
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["propose", "--dry-run"]);
    cmd.assert()
        .success()
        .stdout(contains("dry run"))
        .stdout(contains("local edit"));

    // scoped: --module example
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff", "--module", "example"]);
    cmd.assert().code(1);

    // unknown module: error exit 2
    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff", "--module", "nope"]);
    cmd.assert().failure().stderr(contains("not found"));
}

#[test]
fn write_adopts_existing_file_with_backup() {
    let home = TempDir::new().unwrap();
    init(&home);

    // pre-existing unmanaged file at the target path
    let target = home.path().join(".dot-example.txt");
    fs::write(&target, "precious original").unwrap();

    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["write"]);
    cmd.assert()
        .success()
        .stdout(contains("adopted"))
        .stdout(contains("1 updated"));

    // original preserved under backups/pre-write
    let pre_write = home.path().join(".config/state/dot/backups/pre-write");
    let mut found = false;
    for entry in walkdir::WalkDir::new(&pre_write).into_iter().flatten() {
        if entry.file_type().is_file()
            && fs::read_to_string(entry.path()).unwrap() == "precious original"
        {
            found = true;
        }
    }
    assert!(found, "original not saved to pre-write backup");
}

#[test]
fn disabled_module_is_skipped() {
    let home = TempDir::new().unwrap();
    init(&home);

    let cfg_path = home.path().join(".config/dot/config.toml");
    let mut cfg = fs::read_to_string(&cfg_path).unwrap();
    // init writes the table form `[modules.example] enabled = true`
    cfg = cfg.replace("enabled = true", "enabled = false");
    fs::write(&cfg_path, cfg).unwrap();

    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["diff"]);
    cmd.assert().code(0); // nothing to do

    let mut cmd = dot();
    env_home(&mut cmd, &home);
    cmd.args(["write"]);
    cmd.assert().success().stdout(contains("0 created"));
}
