use std::fs;
use std::path::{Path, PathBuf};

fn discover_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", dir.display(), e))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn every_fixture_parses() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decks");
    let fixtures = discover_fixtures(&dir);

    assert!(
        !fixtures.is_empty(),
        "no fixtures discovered in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    for path in &fixtures {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        if let Err(err) = oxlide::parse_deck(&source) {
            failures.push(format!("{}: {:?}", path.display(), err));
        }
    }

    assert!(
        failures.is_empty(),
        "fixtures failed to parse:\n  {}",
        failures.join("\n  ")
    );
}
