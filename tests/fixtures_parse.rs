use std::fs;
use std::path::{Path, PathBuf};

use oxlide::parser::Block;

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
fn engineering_note_fixture_groups_blocks_into_one_cell() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decks/08-engineering-note.md");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    let deck = oxlide::parse_deck(&source).expect("fixture should parse");

    assert_eq!(deck.slides.len(), 1, "expected single slide");
    assert_eq!(
        deck.slides[0].cells.len(),
        1,
        "contiguous blocks (no blank lines between) should collapse into one cell"
    );
    let blocks = &deck.slides[0].cells[0].blocks;
    assert_eq!(blocks.len(), 4, "expected heading + prose + list + code");
    assert!(matches!(blocks[0], Block::Heading { .. }));
    assert!(matches!(blocks[1], Block::Paragraph { .. }));
    assert!(matches!(blocks[2], Block::List { .. }));
    assert!(matches!(blocks[3], Block::CodeBlock { .. }));
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
