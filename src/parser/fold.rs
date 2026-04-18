use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::ast::{
    Block, Cell, Directive, ImageMeta, InlineSpan, ListItem, ParseError, Slide, SlideDeck,
    SourceSpan,
};
use super::prepass::{
    CELL_BREAK_SENTINEL, NOTES_END_SENTINEL, NOTES_START_SENTINEL, PrepassOutput,
    SLIDE_BREAK_SENTINEL, VISIBLE_END_SENTINEL, VISIBLE_START_SENTINEL, entry_containing,
    rewritten_to_original,
};

fn parse_oxlide_comment(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let inner = trimmed.strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let after_prefix = inner.strip_prefix("oxlide-")?;
    let colon_pos = after_prefix.find(':')?;
    let name = &after_prefix[..colon_pos];
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let args = after_prefix[colon_pos + 1..].trim().to_string();
    Some((name.to_string(), args))
}

fn is_internal_sentinel(name: &str) -> bool {
    matches!(
        name,
        "visible-start"
            | "visible-end"
            | "notes-start"
            | "notes-end"
            | "image-meta"
            | "slide-break"
            | "cell-break"
    )
}

enum Builder {
    Paragraph {
        start_rw: usize,
        spans: Vec<InlineSpan>,
    },
    Heading {
        level: u8,
        start_rw: usize,
        spans: Vec<InlineSpan>,
    },
    List {
        ordered: bool,
        start_rw: usize,
        items: Vec<ListItem>,
    },
    Item {
        start_rw: usize,
        blocks: Vec<Block>,
        pending: Option<PendingInline>,
    },
    Strong {
        spans: Vec<InlineSpan>,
    },
    Emphasis {
        spans: Vec<InlineSpan>,
    },
    Link {
        url: String,
        spans: Vec<InlineSpan>,
    },
    Image {
        url: String,
        spans: Vec<InlineSpan>,
    },
    CodeBlock {
        start_rw: usize,
        lang: Option<String>,
        source: String,
    },
    Unhandled,
}

fn parse_code_lang(info: &str) -> Option<String> {
    let token: String = info
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if token.is_empty() { None } else { Some(token) }
}

fn flatten_inline_to_string(spans: &[InlineSpan]) -> String {
    let mut out = String::new();
    for span in spans {
        match span {
            InlineSpan::Text(s) => out.push_str(s),
            InlineSpan::Code(s) => out.push_str(s),
            InlineSpan::Strong(inner) | InlineSpan::Emphasis(inner) => {
                out.push_str(&flatten_inline_to_string(inner));
            }
            InlineSpan::Link { text, .. } => {
                out.push_str(&flatten_inline_to_string(text));
            }
            InlineSpan::Image { alt, .. } => out.push_str(alt),
        }
    }
    out
}

struct PendingInline {
    start_rw: usize,
    end_rw: usize,
    spans: Vec<InlineSpan>,
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

fn push_block_into_parent(
    stack: &mut [Builder],
    block: Block,
    top_level_blocks: &mut Vec<Block>,
) {
    match stack.last_mut() {
        Some(Builder::Item {
            blocks, pending, ..
        }) => {
            if let Some(pt) = pending.take() {
                blocks.push(Block::Paragraph {
                    spans: pt.spans,
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
            top_level_blocks.push(block);
        }
    }
}

fn inline_container(
    stack: &mut [Builder],
    range_start: usize,
    range_end: usize,
) -> Option<&mut Vec<InlineSpan>> {
    for builder in stack.iter_mut().rev() {
        match builder {
            Builder::Paragraph { spans, .. }
            | Builder::Heading { spans, .. }
            | Builder::Strong { spans }
            | Builder::Emphasis { spans }
            | Builder::Link { spans, .. }
            | Builder::Image { spans, .. } => return Some(spans),
            Builder::Item { pending, .. } => {
                let pt = pending.get_or_insert_with(|| PendingInline {
                    start_rw: range_start,
                    end_rw: range_end,
                    spans: Vec::new(),
                });
                pt.end_rw = range_end;
                return Some(&mut pt.spans);
            }
            Builder::List { .. } | Builder::CodeBlock { .. } | Builder::Unhandled => continue,
        }
    }
    None
}

fn push_inline(
    stack: &mut [Builder],
    span: InlineSpan,
    range_start: usize,
    range_end: usize,
) {
    let Some(container) = inline_container(stack, range_start, range_end) else {
        return;
    };
    if let InlineSpan::Text(text) = span {
        if let Some(InlineSpan::Text(last)) = container.last_mut() {
            last.push_str(&text);
        } else {
            container.push(InlineSpan::Text(text));
        }
    } else {
        container.push(span);
    }
}

fn flush_cell(
    cell_blocks: &mut Vec<Block>,
    cells: &mut Vec<Cell>,
    pending_cell_directives: &mut Vec<Directive>,
) {
    if cell_blocks.is_empty() {
        return;
    }
    let start = cell_blocks[0].span().start;
    let end = cell_blocks.last().unwrap().span().end;
    let taken = std::mem::take(cell_blocks);
    let directives = std::mem::take(pending_cell_directives);
    cells.push(Cell {
        blocks: taken,
        directives,
        span: SourceSpan { start, end },
    });
}

fn flush_slide(
    cells: &mut Vec<Cell>,
    notes: &mut Vec<Block>,
    slides: &mut Vec<Slide>,
    pending_slide_directives: &mut Vec<Directive>,
) {
    if cells.is_empty() && notes.is_empty() {
        return;
    }
    let mut start = usize::MAX;
    let mut end = 0usize;
    for c in cells.iter() {
        if c.span.start < start {
            start = c.span.start;
        }
        if c.span.end > end {
            end = c.span.end;
        }
    }
    for b in notes.iter() {
        let s = b.span();
        if s.start < start {
            start = s.start;
        }
        if s.end > end {
            end = s.end;
        }
    }
    let taken_cells = std::mem::take(cells);
    let taken_notes = std::mem::take(notes);
    let directives = std::mem::take(pending_slide_directives);
    slides.push(Slide {
        cells: taken_cells,
        notes: taken_notes,
        directives,
        span: SourceSpan { start, end },
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockTarget {
    Cell,
    Notes,
}

pub fn fold(prepass_out: &PrepassOutput) -> Result<SlideDeck, ParseError> {
    let parser = Parser::new_ext(&prepass_out.rewritten, Options::empty());
    let entries = &prepass_out.entries;
    let map = |rw: usize| rewritten_to_original(rw, entries);

    let mut stack: Vec<Builder> = Vec::new();
    let mut current_cell_blocks: Vec<Block> = Vec::new();
    let mut current_slide_cells: Vec<Cell> = Vec::new();
    let mut current_slide_notes: Vec<Block> = Vec::new();
    let mut target_stack: Vec<BlockTarget> = vec![BlockTarget::Cell];
    let mut slides: Vec<Slide> = Vec::new();
    let mut pending_cell_directives: Vec<Directive> = Vec::new();
    let mut pending_slide_directives: Vec<Directive> = Vec::new();
    let mut pending_cell_break = false;

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Rule => {
                flush_cell(
                    &mut current_cell_blocks,
                    &mut current_slide_cells,
                    &mut pending_cell_directives,
                );
                flush_slide(
                    &mut current_slide_cells,
                    &mut current_slide_notes,
                    &mut slides,
                    &mut pending_slide_directives,
                );
                target_stack.clear();
                target_stack.push(BlockTarget::Cell);
                pending_cell_break = false;
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                let text = s.as_ref();
                let trimmed = text.trim();
                if trimmed == SLIDE_BREAK_SENTINEL {
                    flush_cell(
                        &mut current_cell_blocks,
                        &mut current_slide_cells,
                        &mut pending_cell_directives,
                    );
                    flush_slide(
                        &mut current_slide_cells,
                        &mut current_slide_notes,
                        &mut slides,
                        &mut pending_slide_directives,
                    );
                    target_stack.clear();
                    target_stack.push(BlockTarget::Cell);
                    pending_cell_break = false;
                } else if trimmed == CELL_BREAK_SENTINEL {
                    if stack.is_empty() {
                        pending_cell_break = true;
                    }
                } else if trimmed == NOTES_START_SENTINEL {
                    if stack.is_empty() {
                        flush_cell(
                            &mut current_cell_blocks,
                            &mut current_slide_cells,
                            &mut pending_cell_directives,
                        );
                        pending_cell_break = false;
                        target_stack.push(BlockTarget::Notes);
                    }
                } else if trimmed == NOTES_END_SENTINEL {
                    if stack.is_empty() && target_stack.len() > 1 {
                        target_stack.pop();
                    }
                } else if trimmed == VISIBLE_START_SENTINEL {
                    if stack.is_empty() {
                        target_stack.push(BlockTarget::Cell);
                    }
                } else if trimmed == VISIBLE_END_SENTINEL {
                    if stack.is_empty() && target_stack.len() > 1 {
                        target_stack.pop();
                    }
                } else if let Some((name, args)) = parse_oxlide_comment(text) {
                    if name == "image-meta" {
                        if let Ok(parsed) = serde_json::from_str::<ImageMeta>(&args)
                            && let Some(Block::Image { meta, span, .. }) =
                                current_cell_blocks.last_mut()
                        {
                            *meta = Some(parsed);
                            if let Some(entry) = entry_containing(range.start, entries) {
                                span.end = entry.orig_end;
                            }
                        }
                    } else if !is_internal_sentinel(&name) {
                        let directive = Directive::Raw {
                            name,
                            args,
                            span: SourceSpan {
                                start: map(range.start),
                                end: map(range.end),
                            },
                        };
                        if !stack.is_empty() || !current_cell_blocks.is_empty() {
                            pending_cell_directives.push(directive);
                        } else {
                            pending_slide_directives.push(directive);
                        }
                    }
                }
            }
            Event::Start(tag) => {
                if matches!(tag, Tag::HtmlBlock) {
                    continue;
                }
                if stack.is_empty() && pending_cell_break {
                    flush_cell(
                        &mut current_cell_blocks,
                        &mut current_slide_cells,
                        &mut pending_cell_directives,
                    );
                    pending_cell_break = false;
                }
                let start_rw = range.start;
                let builder = match tag {
                    Tag::Paragraph => Builder::Paragraph {
                        start_rw,
                        spans: Vec::new(),
                    },
                    Tag::Heading { level, .. } => Builder::Heading {
                        level: heading_level_to_u8(level),
                        start_rw,
                        spans: Vec::new(),
                    },
                    Tag::List(start_num) => Builder::List {
                        ordered: start_num.is_some(),
                        start_rw,
                        items: Vec::new(),
                    },
                    Tag::Item => Builder::Item {
                        start_rw,
                        blocks: Vec::new(),
                        pending: None,
                    },
                    Tag::Strong => Builder::Strong { spans: Vec::new() },
                    Tag::Emphasis => Builder::Emphasis { spans: Vec::new() },
                    Tag::Link { dest_url, .. } => Builder::Link {
                        url: dest_url.into_string(),
                        spans: Vec::new(),
                    },
                    Tag::Image { dest_url, .. } => Builder::Image {
                        url: dest_url.into_string(),
                        spans: Vec::new(),
                    },
                    Tag::CodeBlock(CodeBlockKind::Fenced(info)) => Builder::CodeBlock {
                        start_rw,
                        lang: parse_code_lang(info.as_ref()),
                        source: String::new(),
                    },
                    Tag::CodeBlock(CodeBlockKind::Indented) => {
                        return Err(ParseError::UnsupportedIndentedCodeBlock {
                            span: SourceSpan {
                                start: map(range.start),
                                end: map(range.end),
                            },
                        });
                    }
                    _ => Builder::Unhandled,
                };
                stack.push(builder);
            }
            Event::End(tag_end) => {
                if matches!(tag_end, TagEnd::HtmlBlock) {
                    continue;
                }
                let Some(builder) = stack.pop() else {
                    continue;
                };
                let end_original = map(range.end);
                match (builder, tag_end) {
                    (Builder::Paragraph { start_rw, spans }, TagEnd::Paragraph) => {
                        let span = SourceSpan {
                            start: map(start_rw),
                            end: end_original,
                        };
                        let block = if spans.len() == 1
                            && matches!(spans[0], InlineSpan::Image { .. })
                        {
                            let Some(InlineSpan::Image { src, alt }) = spans.into_iter().next()
                            else {
                                unreachable!()
                            };
                            Block::Image {
                                src,
                                alt,
                                meta: None,
                                span,
                            }
                        } else {
                            Block::Paragraph { spans, span }
                        };
                        if matches!(target_stack.last(), Some(BlockTarget::Notes)) {
                            push_block_into_parent(&mut stack, block, &mut current_slide_notes);
                        } else {
                            push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                        }
                    }
                    (
                        Builder::Heading {
                            level,
                            start_rw,
                            spans,
                        },
                        TagEnd::Heading(_),
                    ) => {
                        let block = Block::Heading {
                            level,
                            spans,
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        if matches!(target_stack.last(), Some(BlockTarget::Notes)) {
                            push_block_into_parent(&mut stack, block, &mut current_slide_notes);
                        } else {
                            push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                        }
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
                        if matches!(target_stack.last(), Some(BlockTarget::Notes)) {
                            push_block_into_parent(&mut stack, block, &mut current_slide_notes);
                        } else {
                            push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                        }
                    }
                    (
                        Builder::Item {
                            start_rw,
                            mut blocks,
                            pending,
                        },
                        TagEnd::Item,
                    ) => {
                        if let Some(pt) = pending {
                            blocks.push(Block::Paragraph {
                                spans: pt.spans,
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
                    (Builder::Strong { spans }, TagEnd::Strong) => {
                        push_inline(
                            &mut stack,
                            InlineSpan::Strong(spans),
                            range.start,
                            range.end,
                        );
                    }
                    (Builder::Emphasis { spans }, TagEnd::Emphasis) => {
                        push_inline(
                            &mut stack,
                            InlineSpan::Emphasis(spans),
                            range.start,
                            range.end,
                        );
                    }
                    (Builder::Link { url, spans }, TagEnd::Link) => {
                        push_inline(
                            &mut stack,
                            InlineSpan::Link { url, text: spans },
                            range.start,
                            range.end,
                        );
                    }
                    (Builder::Image { url, spans }, TagEnd::Image) => {
                        let alt = flatten_inline_to_string(&spans);
                        push_inline(
                            &mut stack,
                            InlineSpan::Image { src: url, alt },
                            range.start,
                            range.end,
                        );
                    }
                    (
                        Builder::CodeBlock {
                            start_rw,
                            lang,
                            source,
                        },
                        TagEnd::CodeBlock,
                    ) => {
                        let block = Block::CodeBlock {
                            lang,
                            source,
                            span: SourceSpan {
                                start: map(start_rw),
                                end: end_original,
                            },
                        };
                        if matches!(target_stack.last(), Some(BlockTarget::Notes)) {
                            push_block_into_parent(&mut stack, block, &mut current_slide_notes);
                        } else {
                            push_block_into_parent(&mut stack, block, &mut current_cell_blocks);
                        }
                    }
                    (Builder::Unhandled, _) => {}
                    _ => {}
                }
            }
            Event::Text(s) => {
                if let Some(Builder::CodeBlock { source, .. }) = stack.last_mut() {
                    source.push_str(s.as_ref());
                } else {
                    push_inline(
                        &mut stack,
                        InlineSpan::Text(s.into_string()),
                        range.start,
                        range.end,
                    );
                }
            }
            Event::Code(s) => {
                push_inline(
                    &mut stack,
                    InlineSpan::Code(s.into_string()),
                    range.start,
                    range.end,
                );
            }
            Event::SoftBreak => {
                push_inline(
                    &mut stack,
                    InlineSpan::Text(" ".to_string()),
                    range.start,
                    range.end,
                );
            }
            Event::HardBreak => {
                push_inline(
                    &mut stack,
                    InlineSpan::Text("\n".to_string()),
                    range.start,
                    range.end,
                );
            }
            _ => {}
        }
    }

    flush_cell(
        &mut current_cell_blocks,
        &mut current_slide_cells,
        &mut pending_cell_directives,
    );
    flush_slide(
        &mut current_slide_cells,
        &mut current_slide_notes,
        &mut slides,
        &mut pending_slide_directives,
    );

    Ok(SlideDeck {
        slides,
        source: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::prepass::prepass;
    use super::*;

    fn fold_source(source: &str) -> SlideDeck {
        fold(&prepass(source).unwrap()).unwrap()
    }

    #[test]
    fn single_paragraph_single_slide() {
        let deck = fold_source("\thello");
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
        let deck = fold_source("\tA\n\n---\n\n\tB");
        assert_eq!(deck.slides.len(), 2);
    }

    #[test]
    fn cell_break_between_paragraphs() {
        let deck = fold_source("\tPara A\n\n\tPara B");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 2);
    }

    #[test]
    fn leading_rule_does_not_produce_empty_slide() {
        let deck = fold_source("---\n\n\thello");
        assert_eq!(deck.slides.len(), 1);
    }

    #[test]
    fn trailing_rule_does_not_produce_empty_slide() {
        let deck = fold_source("\thello\n\n---\n");
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
        let deck = fold_source("\t- a\n\t- b");
        if let Block::List { ordered, items, .. } = &deck.slides[0].cells[0].blocks[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn ordered_list() {
        let deck = fold_source("\t1. a\n\t2. b");
        if let Block::List { ordered, items, .. } = &deck.slides[0].cells[0].blocks[0] {
            assert!(*ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected ordered list");
        }
    }

    #[test]
    fn heading_plus_prose_no_blank_is_one_cell() {
        let deck = fold_source("# Heading\n\tprose");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 1);
        assert_eq!(deck.slides[0].cells[0].blocks.len(), 2);
        assert!(matches!(
            deck.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[0].blocks[1],
            Block::Paragraph { .. }
        ));
    }

    #[test]
    fn heading_blank_prose_is_two_cells() {
        let deck = fold_source("# Heading\n\n\tprose");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 2);
        assert!(matches!(
            deck.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[1].blocks[0],
            Block::Paragraph { .. }
        ));
    }

    #[test]
    fn heading_blank_prose_blank_list_is_three_cells() {
        let deck = fold_source("# Heading\n\n\tprose\n\n\t- item");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 3);
        assert!(matches!(
            deck.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[1].blocks[0],
            Block::Paragraph { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[2].blocks[0],
            Block::List { .. }
        ));
    }

    #[test]
    fn heading_prose_list_no_blanks_is_one_cell_with_three_blocks() {
        let deck = fold_source("# Heading\n\tprose\n\t- item");
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0].cells.len(), 1);
        assert_eq!(deck.slides[0].cells[0].blocks.len(), 3);
        assert!(matches!(
            deck.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[0].blocks[1],
            Block::Paragraph { .. }
        ));
        assert!(matches!(
            deck.slides[0].cells[0].blocks[2],
            Block::List { .. }
        ));
    }
}
