pub mod ast;
pub mod fold;
pub mod prepass;

pub use ast::{
    Block, Cell, Directive, ImageAlign, ImageMeta, ImageSize, InlineSpan, ListItem, ParseError,
    Slide, SlideDeck, SourceSpan,
};

#[cfg(test)]
fn directive_name(d: &Directive) -> &str {
    let Directive::Raw { name, .. } = d;
    name
}

#[cfg(test)]
fn directive_args(d: &Directive) -> &str {
    let Directive::Raw { args, .. } = d;
    args
}

pub fn parse_deck(source: &str) -> Result<SlideDeck, ParseError> {
    let prepass_out = prepass::prepass(source)?;
    let mut deck = fold::fold(&prepass_out)?;
    deck.source = source.to_string();
    Ok(deck)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(source: &str) -> SlideDeck {
        parse_deck(source).expect("parse_deck should not fail in v1")
    }

    #[test]
    fn acceptance_two_slides_with_rule() {
        let src = "# Hello\n\n\tWorld\n\n---\n\n# Two";
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[0].cells.len(), 2, "heading + paragraph as 2 cells");
        assert!(matches!(
            d.slides[0].cells[0].blocks[0],
            Block::Heading { level: 1, .. }
        ));
        assert!(matches!(
            d.slides[0].cells[1].blocks[0],
            Block::Paragraph { .. }
        ));
        assert_eq!(d.slides[1].cells.len(), 1);
        assert!(matches!(
            d.slides[1].cells[0].blocks[0],
            Block::Heading { level: 1, .. }
        ));
    }

    #[test]
    fn acceptance_two_slides_double_blank() {
        let d = deck("# A\n\n\n\n# B");
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn acceptance_cell_break_single_blank() {
        let d = deck("\tPara A\n\n\tPara B");
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 2);
    }

    #[test]
    fn empty_input_produces_empty_deck() {
        let d = deck("");
        assert!(d.slides.is_empty());
        assert_eq!(d.source, "");
    }

    #[test]
    fn only_blank_lines_produces_empty_deck() {
        let d = deck("\n\n\n");
        assert!(d.slides.is_empty());
    }

    #[test]
    fn source_preserved_on_deck() {
        let src = "# Hello";
        let d = deck(src);
        assert_eq!(d.source, src);
    }

    #[test]
    fn heading_levels_one_to_six() {
        for (i, prefix) in ["#", "##", "###", "####", "#####", "######"]
            .iter()
            .enumerate()
        {
            let src = format!("{} heading", prefix);
            let d = deck(&src);
            match &d.slides[0].cells[0].blocks[0] {
                Block::Heading { level, .. } => assert_eq!(*level as usize, i + 1),
                other => panic!("expected heading, got {:?}", other),
            }
        }
    }

    #[test]
    fn spans_reference_original_source_byte_offsets() {
        let src = "# A\n\n\n\n# B";
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
        let h1 = &d.slides[0].cells[0].blocks[0];
        if let Block::Heading { span, .. } = h1 {
            assert_eq!(&src[span.start..span.end].trim_end(), &"# A");
        } else {
            panic!("expected heading");
        }
        let h2 = &d.slides[1].cells[0].blocks[0];
        if let Block::Heading { span, .. } = h2 {
            assert_eq!(&src[span.start..span.end].trim_end(), &"# B");
        } else {
            panic!("expected heading");
        }
    }

    #[test]
    fn nested_list_preserves_structure() {
        let src = "\t- outer\n\t  - inner\n";
        let d = deck(src);
        let outer = &d.slides[0].cells[0].blocks[0];
        if let Block::List {
            ordered: false,
            items,
            ..
        } = outer
        {
            assert_eq!(items.len(), 1);
            let outer_item = &items[0];
            let inner = outer_item
                .blocks
                .iter()
                .find(|b| matches!(b, Block::List { .. }));
            assert!(inner.is_some(), "outer item should contain nested list");
        } else {
            panic!("expected outer unordered list, got {:?}", outer);
        }
    }

    #[test]
    fn single_slide_when_no_break() {
        let d = deck("# heading\n\n\tpara\n\n\tpara2\n");
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 3);
    }

    #[test]
    fn consecutive_rules_do_not_emit_empty_slides() {
        let d = deck("# A\n\n---\n\n---\n\n# B");
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_multi_slide_rule() {
        let src = include_str!("fixtures/multi-slide-rule.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_multi_slide_double_blank() {
        let src = include_str!("fixtures/multi-slide-double-blank.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_cell_break_single_blank() {
        let src = include_str!("fixtures/cell-break-single-blank.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 2);
    }

    #[test]
    fn fixture_single_slide_paragraph() {
        let src = include_str!("fixtures/single-slide-paragraph.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 1);
    }

    #[test]
    fn fixture_single_slide_heading() {
        let src = include_str!("fixtures/single-slide-heading.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert!(matches!(
            d.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
    }

    #[test]
    fn fixture_nested_list() {
        let src = include_str!("fixtures/nested-list.md");
        let d = deck(src);
        assert!(matches!(d.slides[0].cells[0].blocks[0], Block::List { .. }));
    }

    #[test]
    fn fixture_ordered_list() {
        let src = include_str!("fixtures/ordered-list.md");
        let d = deck(src);
        if let Block::List { ordered, .. } = &d.slides[0].cells[0].blocks[0] {
            assert!(ordered);
        } else {
            panic!("expected ordered list");
        }
    }

    #[test]
    fn fixture_empty_input() {
        let src = include_str!("fixtures/empty-input.md");
        let d = deck(src);
        assert!(d.slides.is_empty());
    }

    #[test]
    fn fixture_leading_trailing_rule() {
        let src = include_str!("fixtures/leading-trailing-rule.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
    }

    #[test]
    fn fixture_directive_on_slide() {
        // Directive at the top of the file is captured as deck-level.
        let src = include_str!("fixtures/directive-on-slide.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.directives.len(), 1);
        assert_eq!(super::directive_name(&d.directives[0]), "fx");
        assert_eq!(
            super::directive_args(&d.directives[0]),
            "fade duration=300"
        );
        assert!(d.slides[0].directives.is_empty());
        for cell in &d.slides[0].cells {
            assert!(cell.directives.is_empty());
        }
    }

    #[test]
    fn fixture_directive_on_cell() {
        let src = include_str!("fixtures/directive-on-cell.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert!(d.slides[0].directives.is_empty());
        let with_directive = d.slides[0]
            .cells
            .iter()
            .find(|c| !c.directives.is_empty())
            .expect("expected a cell with a directive");
        assert_eq!(with_directive.directives.len(), 1);
        assert_eq!(
            super::directive_name(&with_directive.directives[0]),
            "layout"
        );
        assert_eq!(
            super::directive_args(&with_directive.directives[0]),
            "title-ascii"
        );
    }

    #[test]
    fn fixture_directive_empty_args() {
        let src = include_str!("fixtures/directive-empty-args.md");
        let d = deck(src);
        assert_eq!(d.directives.len(), 1);
        assert_eq!(super::directive_name(&d.directives[0]), "fx");
        assert_eq!(super::directive_args(&d.directives[0]), "");
    }

    #[test]
    fn fixture_directive_multiple_in_source_order() {
        let src = include_str!("fixtures/directive-multiple.md");
        let d = deck(src);
        let all: Vec<&Directive> = d
            .directives
            .iter()
            .chain(d.slides[0].directives.iter())
            .chain(d.slides[0].cells.iter().flat_map(|c| c.directives.iter()))
            .collect();
        assert_eq!(all.len(), 2);
        let names: Vec<&str> = all.iter().map(|d| super::directive_name(d)).collect();
        assert_eq!(names, vec!["fx", "layout"]);
    }

    #[test]
    fn fixture_non_oxlide_comment_ignored() {
        let src = include_str!("fixtures/non-oxlide-comment-ignored.md");
        let d = deck(src);
        assert!(d.slides[0].directives.is_empty());
        for cell in &d.slides[0].cells {
            assert!(cell.directives.is_empty());
        }
    }

    #[test]
    fn fixture_internal_sentinel_not_leaked() {
        let src = include_str!("fixtures/internal-sentinel-not-leaked.md");
        let d = deck(src);
        assert!(d.slides[0].directives.is_empty());
        for cell in &d.slides[0].cells {
            assert!(cell.directives.is_empty());
        }
    }

    #[test]
    fn fixture_directive_whitespace_tolerance() {
        let src = include_str!("fixtures/directive-whitespace-tolerance.md");
        let d = deck(src);
        let all: Vec<&Directive> = d
            .directives
            .iter()
            .chain(d.slides[0].directives.iter())
            .chain(d.slides[0].cells.iter().flat_map(|c| c.directives.iter()))
            .collect();
        assert_eq!(all.len(), 2);
        for directive in &all {
            assert_eq!(super::directive_name(directive), "fx");
            assert_eq!(super::directive_args(directive), "fade");
        }
    }

    #[test]
    fn fixture_directive_with_hyphens() {
        let src = include_str!("fixtures/directive-with-hyphens.md");
        let d = deck(src);
        assert_eq!(d.directives.len(), 1);
        assert_eq!(
            super::directive_name(&d.directives[0]),
            "image-meta-disable"
        );
        assert_eq!(super::directive_args(&d.directives[0]), "true");
    }

    #[test]
    fn directive_span_points_into_original_source() {
        let src = "<!-- oxlide-fx: fade -->\n\n# Slide";
        let d = deck(src);
        let dir = &d.directives[0];
        let Directive::Raw { span, .. } = dir;
        let slice = &src[span.start..span.end];
        assert!(
            slice.contains("oxlide-fx"),
            "expected span to cover comment, got {:?}",
            slice
        );
    }

    fn first_paragraph_spans(d: &SlideDeck) -> &[InlineSpan] {
        match &d.slides[0].cells[0].blocks[0] {
            Block::Paragraph { spans, .. } => spans,
            other => panic!("expected first block to be paragraph, got {:?}", other),
        }
    }

    #[test]
    fn fixture_inline_bold() {
        let src = include_str!("fixtures/inline-bold.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Strong(vec![InlineSpan::Text("hello".into())])]
        );
    }

    #[test]
    fn fixture_inline_italic() {
        let src = include_str!("fixtures/inline-italic.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Emphasis(vec![InlineSpan::Text("italic".into())])]
        );
    }

    #[test]
    fn fixture_inline_code() {
        let src = include_str!("fixtures/inline-code.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[
                InlineSpan::Text("plain ".into()),
                InlineSpan::Code("code".into()),
                InlineSpan::Text(" here".into()),
            ]
        );
    }

    #[test]
    fn fixture_inline_link() {
        let src = include_str!("fixtures/inline-link.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Link {
                url: "https://x.com".into(),
                text: vec![InlineSpan::Text("click".into())],
            }]
        );
    }

    #[test]
    fn fixture_nested_bold_italic() {
        let src = include_str!("fixtures/nested-bold-italic.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Strong(vec![InlineSpan::Emphasis(vec![
                InlineSpan::Text("a".into())
            ])])]
        );
    }

    #[test]
    fn fixture_autolink() {
        let src = include_str!("fixtures/autolink.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Link {
                url: "https://example.com".into(),
                text: vec![InlineSpan::Text("https://example.com".into())],
            }]
        );
    }

    #[test]
    fn fixture_link_with_code_text() {
        let src = include_str!("fixtures/link-with-code-text.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[InlineSpan::Link {
                url: "https://x.com".into(),
                text: vec![InlineSpan::Code("code".into())],
            }]
        );
    }

    #[test]
    fn fixture_hard_break() {
        let src = include_str!("fixtures/hard-break.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(spans, &[InlineSpan::Text("foo\nbar".into())]);
    }

    #[test]
    fn fixture_soft_break_coalesced() {
        let src = include_str!("fixtures/soft-break-coalesced.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(spans, &[InlineSpan::Text("foo bar".into())]);
    }

    #[test]
    fn fixture_adjacent_text_coalesced() {
        let src = include_str!("fixtures/adjacent-text-coalesced.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(spans, &[InlineSpan::Text("foo*bar".into())]);
    }

    #[test]
    fn fixture_block_image() {
        let src = include_str!("fixtures/block-image.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { src, alt, .. } => {
                assert_eq!(src, "logo.png");
                assert_eq!(alt, "logo");
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_inline_image() {
        let src = include_str!("fixtures/inline-image.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[
                InlineSpan::Text("Look at ".into()),
                InlineSpan::Image {
                    src: "pic.png".into(),
                    alt: "this".into(),
                },
                InlineSpan::Text(" thing".into()),
            ]
        );
    }

    #[test]
    fn fixture_image_no_alt() {
        let src = include_str!("fixtures/image-no-alt.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { src, alt, .. } => {
                assert_eq!(src, "logo.png");
                assert_eq!(alt, "");
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_image_with_title_ignored() {
        let src = include_str!("fixtures/image-with-title-ignored.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { src, alt, .. } => {
                assert_eq!(src, "path.png");
                assert_eq!(alt, "alt");
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_multiple_images_inline() {
        let src = include_str!("fixtures/multiple-images-inline.md");
        let d = deck(src);
        let spans = first_paragraph_spans(&d);
        assert_eq!(
            spans,
            &[
                InlineSpan::Image {
                    src: "a.png".into(),
                    alt: "a".into(),
                },
                InlineSpan::Text(" ".into()),
                InlineSpan::Image {
                    src: "b.png".into(),
                    alt: "b".into(),
                },
            ]
        );
    }

    #[test]
    fn fixture_image_in_heading() {
        let src = include_str!("fixtures/image-in-heading.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Heading { level, spans, .. } => {
                assert_eq!(*level, 1);
                assert_eq!(
                    spans,
                    &[
                        InlineSpan::Text("Title ".into()),
                        InlineSpan::Image {
                            src: "logo.png".into(),
                            alt: "logo".into(),
                        },
                    ]
                );
            }
            other => panic!("expected heading, got {:?}", other),
        }
    }

    #[test]
    fn fixture_image_with_surrounding_whitespace_stays_paragraph() {
        let src = include_str!("fixtures/image-with-surrounding-whitespace-stays-paragraph.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Paragraph { spans, .. } => {
                let has_image = spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Image { .. }));
                assert!(has_image, "expected an inline image span");
                assert!(
                    spans.len() > 1,
                    "expected paragraph to have multiple spans, got {:?}",
                    spans
                );
            }
            other => panic!("expected Block::Paragraph (not promoted), got {:?}", other),
        }
    }

    #[test]
    fn heading_preserves_inline_spans() {
        let d = deck("## **bold** heading");
        match &d.slides[0].cells[0].blocks[0] {
            Block::Heading { level, spans, .. } => {
                assert_eq!(*level, 2);
                assert_eq!(
                    spans,
                    &[
                        InlineSpan::Strong(vec![InlineSpan::Text("bold".into())]),
                        InlineSpan::Text(" heading".into()),
                    ]
                );
            }
            other => panic!("expected heading, got {:?}", other),
        }
    }

    #[test]
    fn list_item_preserves_inline_spans() {
        let d = deck("\t- plain `code` text");
        match &d.slides[0].cells[0].blocks[0] {
            Block::List { items, .. } => {
                let item = &items[0];
                match &item.blocks[0] {
                    Block::Paragraph { spans, .. } => {
                        assert_eq!(
                            spans,
                            &[
                                InlineSpan::Text("plain ".into()),
                                InlineSpan::Code("code".into()),
                                InlineSpan::Text(" text".into()),
                            ]
                        );
                    }
                    other => panic!("expected paragraph in item, got {:?}", other),
                }
            }
            other => panic!("expected list, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_with_all_keys() {
        let src = include_str!("fixtures/ia-image-with-all-keys.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image {
                src, alt, meta, ..
            } => {
                assert_eq!(src, "hero.png");
                assert_eq!(alt, "");
                let meta = meta.as_ref().expect("meta should be attached");
                assert_eq!(meta.size, Some(ImageSize::Contain));
                assert_eq!(meta.x, Some(ImageAlign::End));
                assert_eq!(meta.y, Some(ImageAlign::Start));
                assert_eq!(meta.background.as_deref(), Some("#fff"));
                assert_eq!(meta.opacity, Some(0.5));
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_no_metadata() {
        let src = include_str!("fixtures/ia-image-no-metadata.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image {
                src, alt, meta, ..
            } => {
                assert_eq!(src, "hero.png");
                assert_eq!(alt, "");
                assert!(meta.is_none());
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_quoted_values() {
        let src = include_str!("fixtures/ia-image-quoted-values.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { meta, .. } => {
                let meta = meta.as_ref().expect("meta should be attached");
                assert_eq!(meta.background.as_deref(), Some("#fff"));
                assert_eq!(meta.size, Some(ImageSize::Contain));
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_unknown_key_ignored() {
        let src = include_str!("fixtures/ia-image-unknown-key-ignored.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { meta, .. } => {
                let meta = meta.as_ref().expect("meta should be attached");
                assert_eq!(meta.size, Some(ImageSize::Contain));
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_invalid_opacity_errors() {
        let src = include_str!("fixtures/ia-image-invalid-opacity-errors.md");
        let err = parse_deck(src).expect_err("should error on invalid opacity");
        match err {
            ParseError::InvalidImageMeta { key, value, .. } => {
                assert_eq!(key, "opacity");
                assert_eq!(value, "1.5");
            }
            other => panic!("expected InvalidImageMeta, got {:?}", other),
        }
    }

    #[test]
    fn fixture_ia_image_non_image_extension_falls_through() {
        let src = include_str!("fixtures/ia-image-non-image-extension-falls-through.md");
        let d = deck(src);
        let has_image_block = d.slides[0]
            .cells
            .iter()
            .flat_map(|c| &c.blocks)
            .any(|b| matches!(b, Block::Image { .. }));
        assert!(
            !has_image_block,
            "non-image extension should not produce Block::Image"
        );
    }

    #[test]
    fn fixture_ia_image_kebab_case_size() {
        let src = include_str!("fixtures/ia-image-kebab-case-size.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::Image { meta, .. } => {
                let meta = meta.as_ref().expect("meta should be attached");
                assert_eq!(meta.size, Some(ImageSize::FitWidth));
            }
            other => panic!("expected Block::Image, got {:?}", other),
        }
    }

    #[test]
    fn fixture_code_with_lang() {
        let src = include_str!("fixtures/code-with-lang.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { lang, source, span } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(source, "fn main() {}\n");
                let slice = &src[span.start..span.end];
                assert!(slice.starts_with("```rust"), "span should start at opening fence, got {:?}", slice);
                assert!(slice.contains("```\n") || slice.ends_with("```"), "span should cover closing fence, got {:?}", slice);
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_code_no_lang() {
        let src = include_str!("fixtures/code-no-lang.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { lang, source, .. } => {
                assert!(lang.is_none());
                assert_eq!(source, "plain text\n");
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_empty_code() {
        let src = include_str!("fixtures/empty-code.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { lang, source, .. } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(source, "");
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_code_with_metadata_suffix() {
        let src = include_str!("fixtures/code-with-metadata-suffix.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { lang, source, .. } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(source, "fn main() {}\n");
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_tilde_fences() {
        let src = include_str!("fixtures/tilde-fences.md");
        let d = deck(src);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { lang, source, .. } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(source, "fn main() {}\n");
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_code_with_blank_lines_inside() {
        let src = include_str!("fixtures/code-with-blank-lines-inside.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1, "blank line and --- inside fence must not split slides");
        assert_eq!(d.slides[0].cells.len(), 1);
        match &d.slides[0].cells[0].blocks[0] {
            Block::CodeBlock { source, .. } => {
                assert_eq!(source, "line 1\n\nline 3\n---\nline 5\n");
            }
            other => panic!("expected Block::CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn fixture_indented_block_errors() {
        let src = include_str!("fixtures/indented-block-errors.md");
        let err = parse_deck(src).expect_err("indented code block should error");
        assert!(matches!(err, ParseError::UnsupportedIndentedCodeBlock { .. }));
    }

    #[test]
    fn slide_break_sentinel_does_not_appear_as_directive() {
        let d = deck("# A\n\n\n\n# B");
        assert_eq!(d.slides.len(), 2);
        for slide in &d.slides {
            assert!(slide.directives.is_empty());
            for cell in &slide.cells {
                assert!(cell.directives.is_empty());
            }
        }
    }

    #[test]
    fn acceptance_notes_inversion_mixed() {
        let src = "# Title\n\nThis is speaker notes.\n\n\tThis is a visible paragraph.\n";
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        let slide = &d.slides[0];
        assert_eq!(slide.cells.len(), 2);
        match &slide.cells[0].blocks[0] {
            Block::Heading { level, spans, .. } => {
                assert_eq!(*level, 1);
                assert_eq!(spans, &[InlineSpan::Text("Title".into())]);
            }
            other => panic!("expected heading, got {:?}", other),
        }
        match &slide.cells[1].blocks[0] {
            Block::Paragraph { spans, .. } => {
                assert_eq!(spans, &[InlineSpan::Text("This is a visible paragraph.".into())]);
            }
            other => panic!("expected visible paragraph, got {:?}", other),
        }
        assert_eq!(slide.notes.len(), 1);
        match &slide.notes[0] {
            Block::Paragraph { spans, .. } => {
                assert_eq!(spans, &[InlineSpan::Text("This is speaker notes.".into())]);
            }
            other => panic!("expected notes paragraph, got {:?}", other),
        }
    }

    #[test]
    fn fixture_notes_only_slide() {
        let src = include_str!("fixtures/notes-only-slide.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert!(d.slides[0].cells.is_empty());
        assert_eq!(d.slides[0].notes.len(), 2);
    }

    #[test]
    fn fixture_mixed_notes_and_visible() {
        let src = include_str!("fixtures/mixed-notes-and-visible.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        let slide = &d.slides[0];
        assert_eq!(slide.cells.len(), 2);
        assert!(matches!(
            slide.cells[0].blocks[0],
            Block::Heading { level: 1, .. }
        ));
        assert!(matches!(
            slide.cells[1].blocks[0],
            Block::Paragraph { .. }
        ));
        assert_eq!(slide.notes.len(), 1);
    }

    #[test]
    fn fixture_all_visible_tab_indent() {
        let src = include_str!("fixtures/all-visible-tab-indent.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 3);
        assert!(d.slides[0].notes.is_empty());
    }

    #[test]
    fn fixture_fenced_code_not_inverted() {
        let src = include_str!("fixtures/fenced-code-not-inverted.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 1);
        assert!(matches!(
            d.slides[0].cells[0].blocks[0],
            Block::CodeBlock { .. }
        ));
        assert!(d.slides[0].notes.is_empty());
    }

    #[test]
    fn fixture_list_tab_indent() {
        let src = include_str!("fixtures/list-tab-indent.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 1);
        match &d.slides[0].cells[0].blocks[0] {
            Block::List { items, ordered, .. } => {
                assert!(!ordered);
                assert_eq!(items.len(), 3);
            }
            other => panic!("expected visible list, got {:?}", other),
        }
        assert!(d.slides[0].notes.is_empty());
    }

    #[test]
    fn fixture_list_mixed_indent() {
        let src = include_str!("fixtures/list-mixed-indent.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 1);
        match &d.slides[0].cells[0].blocks[0] {
            Block::List { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected visible list, got {:?}", other),
        }
        assert_eq!(d.slides[0].notes.len(), 1);
        match &d.slides[0].notes[0] {
            Block::List { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected notes list, got {:?}", other),
        }
    }

    #[test]
    fn fixture_edge_mixed_leading_whitespace() {
        let src = include_str!("fixtures/edge-mixed-leading-whitespace.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert!(d.slides[0].cells.is_empty());
        assert_eq!(d.slides[0].notes.len(), 1);
    }

    #[test]
    fn fixture_span_points_to_original_source() {
        let src = include_str!("fixtures/span-points-to-original-source.md");
        let d = deck(src);
        let slide = &d.slides[0];
        // Heading span points into original
        let heading = &slide.cells[0].blocks[0];
        if let Block::Heading { span, .. } = heading {
            assert!(src[span.start..span.end].contains("Heading"));
        } else {
            panic!("expected heading");
        }
        // Notes span points into original
        assert_eq!(slide.notes.len(), 1);
        let note = &slide.notes[0];
        if let Block::Paragraph { span, .. } = note {
            assert!(src[span.start..span.end].contains("quick brown fox"));
        } else {
            panic!("expected notes paragraph");
        }
        // Visible paragraph span points into original (includes original tab)
        let visible = &slide.cells[1].blocks[0];
        if let Block::Paragraph { span, .. } = visible {
            assert!(src[span.start..span.end].contains("Visible paragraph"));
        } else {
            panic!("expected visible paragraph");
        }
    }
}
