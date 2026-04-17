use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::ast::{Block, Cell, InlineSpan, ListItem, Slide, SlideDeck, SourceSpan};
use super::prepass::{PrepassOutput, SLIDE_BREAK_SENTINEL, rewritten_to_original};

enum Builder {
    Paragraph {
        start_rw: usize,
        text: String,
    },
    Heading {
        level: u8,
        start_rw: usize,
        text: String,
    },
    List {
        ordered: bool,
        start_rw: usize,
        items: Vec<ListItem>,
    },
    Item {
        start_rw: usize,
        blocks: Vec<Block>,
        pending_text: Option<PendingText>,
    },
    Unhandled,
}

struct PendingText {
    start_rw: usize,
    end_rw: usize,
    text: String,
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn spans_from_text(text: String) -> Vec<InlineSpan> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![InlineSpan::Text(text)]
    }
}

fn push_block_into_parent(
    stack: &mut [Builder],
    block: Block,
    current_cell_blocks: &mut Vec<Block>,
) {
    match stack.last_mut() {
        Some(Builder::Item {
            blocks,
            pending_text,
            ..
        }) => {
            if let Some(pt) = pending_text.take() {
                blocks.push(Block::Paragraph {
                    spans: spans_from_text(pt.text),
                    span: SourceSpan {
                        start: pt.start_rw,
                        end: pt.end_rw,
                    },
                });
            }
            blocks.push(block);
        }
        Some(_) => {
            // Blocks can only nest directly inside Items. Anything else is unexpected;
            // drop to avoid corrupting structure.
        }
        None => {
            current_cell_blocks.push(block);
        }
    }
}

fn append_text(stack: &mut [Builder], text: &str, range_start: usize, range_end: usize) {
    for builder in stack.iter_mut().rev() {
        match builder {
            Builder::Paragraph { text: buf, .. } | Builder::Heading { text: buf, .. } => {
                buf.push_str(text);
                return;
            }
            Builder::Item { pending_text, .. } => {
                match pending_text {
                    Some(pt) => {
                        pt.text.push_str(text);
                        pt.end_rw = range_end;
                    }
                    None => {
                        *pending_text = Some(PendingText {
                            start_rw: range_start,
                            end_rw: range_end,
                            text: text.to_string(),
                        });
                    }
                }
                return;
            }
            Builder::List { .. } | Builder::Unhandled => continue,
        }
    }
}

fn flush_cell(cell_blocks: &mut Vec<Block>, cells: &mut Vec<Cell>) {
    if cell_blocks.is_empty() {
        return;
    }
    let start = cell_blocks[0].span().start;
    let end = cell_blocks.last().unwrap().span().end;
    let taken = std::mem::take(cell_blocks);
    cells.push(Cell {
        blocks: taken,
        directives: Vec::new(),
        span: SourceSpan { start, end },
    });
}

fn flush_slide(cells: &mut Vec<Cell>, slides: &mut Vec<Slide>) {
    if cells.is_empty() {
        return;
    }
    let start = cells[0].span.start;
    let end = cells.last().unwrap().span.end;
    let taken = std::mem::take(cells);
    slides.push(Slide {
        cells: taken,
        notes: Vec::new(),
        directives: Vec::new(),
        span: SourceSpan { start, end },
    });
}

pub fn fold(prepass_out: &PrepassOutput) -> SlideDeck {
    let parser = Parser::new_ext(&prepass_out.rewritten, Options::empty());
    let insertions = &prepass_out.insertions;
    let map = |rw: usize| rewritten_to_original(rw, insertions);

    let mut stack: Vec<Builder> = Vec::new();
    let mut current_cell_blocks: Vec<Block> = Vec::new();
    let mut current_slide_cells: Vec<Cell> = Vec::new();
    let mut slides: Vec<Slide> = Vec::new();

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Rule => {
                flush_cell(&mut current_cell_blocks, &mut current_slide_cells);
                flush_slide(&mut current_slide_cells, &mut slides);
            }
            Event::Html(ref s) | Event::InlineHtml(ref s) if s.trim() == SLIDE_BREAK_SENTINEL => {
                flush_cell(&mut current_cell_blocks, &mut current_slide_cells);
                flush_slide(&mut current_slide_cells, &mut slides);
            }
            Event::Start(tag) => {
                if stack.is_empty() && !current_cell_blocks.is_empty() {
                    flush_cell(&mut current_cell_blocks, &mut current_slide_cells);
                }
                let start_rw = range.start;
                let builder = match tag {
                    Tag::Paragraph => Builder::Paragraph {
                        start_rw,
                        text: String::new(),
                    },
                    Tag::Heading { level, .. } => Builder::Heading {
                        level: heading_level_to_u8(level),
                        start_rw,
                        text: String::new(),
                    },
                    Tag::List(start_num) => Builder::List {
                        ordered: start_num.is_some(),
                        start_rw,
                        items: Vec::new(),
                    },
                    Tag::Item => Builder::Item {
                        start_rw,
                        blocks: Vec::new(),
                        pending_text: None,
                    },
                    _ => Builder::Unhandled,
                };
                stack.push(builder);
            }
            Event::End(tag_end) => {
                let Some(builder) = stack.pop() else {
                    continue;
                };
                let end_original = map(range.end);
                match (builder, tag_end) {
                    (Builder::Paragraph { start_rw, text }, TagEnd::Paragraph) => {
                        let block = Block::Paragraph {
                            spans: spans_from_text(text),
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                    }
                    (
                        Builder::Heading {
                            level,
                            start_rw,
                            text,
                        },
                        TagEnd::Heading(_),
                    ) => {
                        let block = Block::Heading {
                            level,
                            spans: spans_from_text(text),
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                    }
                    (
                        Builder::List {
                            ordered,
                            start_rw,
                            items,
                        },
                        TagEnd::List(_),
                    ) => {
                        let block = Block::List {
                            ordered,
                            items,
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                    }
                    (
                        Builder::Item {
                            start_rw,
                            mut blocks,
                            pending_text,
                        },
                        TagEnd::Item,
                    ) => {
                        if let Some(pt) = pending_text {
                            blocks.push(Block::Paragraph {
                                spans: spans_from_text(pt.text),
                                span: SourceSpan {
                                    start: map(pt.start_rw),
                                    end: map(pt.end_rw),
                                },
                            });
                        }
                        let item = ListItem {
                            blocks,
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        if let Some(Builder::List { items, .. }) = stack.last_mut() {
                            items.push(item);
                        }
                    }
                    (Builder::Unhandled, _) => {}
                    _ => {}
                }
            }
            Event::Text(s) => {
                append_text(&mut stack, &s, range.start, range.end);
            }
            Event::SoftBreak => {
                append_text(&mut stack, " ", range.start, range.end);
            }
            Event::HardBreak => {
                append_text(&mut stack, "\n", range.start, range.end);
            }
            _ => {}
        }
    }

    flush_cell(&mut current_cell_blocks, &mut current_slide_cells);
    flush_slide(&mut current_slide_cells, &mut slides);

    SlideDeck {
        slides,
        source: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::prepass::prepass;
    use super::*;

    fn fold_source(source: &str) -> SlideDeck {
        fold(&prepass(source))
    }

    #[test]
    fn single_paragraph_single_slide() {
        let deck = fold_source("hello");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 1);
        assert_eq!(deck.slides[0].cells[0].blocks.len(), 1);
        assert!(matches!(
            deck.slides[0].cells[0].blocks[0],
            Block::Paragraph { .. }
        ));
    }

    #[test]
    fn rule_produces_two_slides() {
        let deck = fold_source("A\n\n---\n\nB");
        assert_eq!(deck.slides.len(), 2);
    }

    #[test]
    fn cell_break_between_paragraphs() {
        let deck = fold_source("Para A\n\nPara B");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 2);
    }

    #[test]
    fn leading_rule_does_not_produce_empty_slide() {
        let deck = fold_source("---\n\nhello");
        assert_eq!(deck.slides.len(), 1);
    }

    #[test]
    fn trailing_rule_does_not_produce_empty_slide() {
        let deck = fold_source("hello\n\n---\n");
        assert_eq!(deck.slides.len(), 1);
    }

    #[test]
    fn heading_level_preserved() {
        let deck = fold_source("### three");
        if let Block::Heading { level, .. } = &deck.slides[0].cells[0].blocks[0] {
            assert_eq!(*level, 3);
        } else {
            panic!("expected heading");
        }
    }

    #[test]
    fn unordered_list() {
        let deck = fold_source("- a\n- b");
        if let Block::List { ordered, items, .. } = &deck.slides[0].cells[0].blocks[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn ordered_list() {
        let deck = fold_source("1. a\n2. b");
        if let Block::List { ordered, items, .. } = &deck.slides[0].cells[0].blocks[0] {
            assert!(*ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected ordered list");
        }
    }
}
