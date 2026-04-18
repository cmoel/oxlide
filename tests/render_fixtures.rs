use std::fs;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use oxlide::parser::{Block, InlineSpan, SlideDeck};
use oxlide::render::{Theme, render_slide};

const CANVAS_WIDTH: u16 = 120;
const CANVAS_HEIGHT: u16 = 40;

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

fn buffer_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

fn has_non_space_cell(buf: &Buffer) -> bool {
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if !buf[(x, y)].symbol().trim().is_empty() {
                return true;
            }
        }
    }
    false
}

fn collect_visible_words(deck: &SlideDeck) -> Vec<String> {
    let mut out = Vec::new();
    for slide in &deck.slides {
        for cell in &slide.cells {
            collect_block_words(&cell.blocks, &mut out);
        }
    }
    out
}

fn collect_block_words(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Heading { spans, .. } | Block::Paragraph { spans, .. } => {
                collect_span_words(spans, out);
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_block_words(&item.blocks, out);
                }
            }
            Block::Image { alt, src, .. } => {
                push_words(alt, out);
                push_words(src, out);
            }
            Block::CodeBlock { source, .. } => {
                push_words(source, out);
            }
        }
    }
}

fn collect_span_words(spans: &[InlineSpan], out: &mut Vec<String>) {
    for span in spans {
        match span {
            InlineSpan::Text(t) | InlineSpan::Code(t) => push_words(t, out),
            InlineSpan::Strong(c) | InlineSpan::Emphasis(c) => collect_span_words(c, out),
            InlineSpan::Link { text, .. } => collect_span_words(text, out),
            InlineSpan::Image { alt, .. } => push_words(alt, out),
        }
    }
}

fn push_words(text: &str, out: &mut Vec<String>) {
    for word in text.split_whitespace() {
        let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
        if cleaned.chars().count() >= 4 {
            out.push(cleaned);
        }
    }
}

#[test]
fn every_fixture_renders() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decks");
    assert!(
        dir.is_dir(),
        "fixture directory not found: {}",
        dir.display()
    );

    let fixtures = discover_fixtures(&dir);
    assert!(
        !fixtures.is_empty(),
        "no fixtures discovered in {}",
        dir.display()
    );

    let theme = Theme::default();
    let area = Rect::new(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    let mut failures: Vec<String> = Vec::new();
    for path in &fixtures {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{}: failed to read: {}", name, e));

        let deck = match oxlide::parse_deck(&source) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: parse failed: {}", name, e));
                continue;
            }
        };

        if deck.slides.is_empty() {
            failures.push(format!("{}: parsed deck has zero slides", name));
            continue;
        }

        let mut any_non_empty = false;
        let mut combined = String::new();
        for slide in &deck.slides {
            let mut buf = Buffer::empty(area);
            render_slide(slide, area, &mut buf, &theme);
            if has_non_space_cell(&buf) {
                any_non_empty = true;
            }
            combined.push_str(&buffer_text(&buf));
        }

        if !any_non_empty {
            failures.push(format!("{}: all slides rendered empty buffers", name));
            continue;
        }

        let words = collect_visible_words(&deck);
        if !words.is_empty() {
            let found = words.iter().any(|w| combined.contains(w.as_str()));
            if !found {
                let sample: Vec<&str> =
                    words.iter().take(5).map(String::as_str).collect();
                failures.push(format!(
                    "{}: no visible word from the source appeared in any rendered buffer; tried {:?}",
                    name, sample
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "render fixtures failed:\n  {}",
        failures.join("\n  ")
    );
}
