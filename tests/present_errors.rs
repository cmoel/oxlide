use std::fs;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_oxlide")
}

#[test]
fn missing_file_exits_non_zero_with_stderr() {
    let output = Command::new(bin())
        .arg("tests/fixtures/does_not_exist.md")
        .output()
        .expect("failed to spawn oxlide");

    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reading deck") || stderr.contains("does_not_exist"),
        "stderr should mention the failing read; got {:?}",
        stderr
    );
}

#[test]
fn empty_deck_exits_non_zero_with_stderr() {
    let tmp = std::env::temp_dir().join(format!("oxlide-empty-{}.md", std::process::id()));
    fs::write(&tmp, "").expect("write tmp file");

    let output = Command::new(bin())
        .arg(&tmp)
        .output()
        .expect("failed to spawn oxlide");

    let _ = fs::remove_file(&tmp);

    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("deck has no slides"),
        "stderr should mention empty deck; got {:?}",
        stderr
    );
}
