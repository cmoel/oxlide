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
    let mut deck = fold::fold(&prepass_out);
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
        let src = "# Hello\n\nWorld\n\n---\n\n# Two";
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
        let d = deck("Para A\n\nPara B");
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
        let src = "- outer\n  - inner\n";
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
        let d = deck("# heading\n\npara\n\npara2\n");
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
        let src = include_str!("fixtures/directive-on-slide.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].directives.len(), 1);
        assert_eq!(super::directive_name(&d.slides[0].directives[0]), "fx");
        assert_eq!(
            super::directive_args(&d.slides[0].directives[0]),
            "fade duration=300"
        );
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
        assert_eq!(d.slides[0].directives.len(), 1);
        assert_eq!(super::directive_name(&d.slides[0].directives[0]), "fx");
        assert_eq!(super::directive_args(&d.slides[0].directives[0]), "");
    }

    #[test]
    fn fixture_directive_multiple_in_source_order() {
        let src = include_str!("fixtures/directive-multiple.md");
        let d = deck(src);
        let all: Vec<&Directive> = d.slides[0]
            .directives
            .iter()
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
        let all: Vec<&Directive> = d.slides[0]
            .directives
            .iter()
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
        assert_eq!(d.slides[0].directives.len(), 1);
        assert_eq!(
            super::directive_name(&d.slides[0].directives[0]),
            "image-meta-disable"
        );
        assert_eq!(super::directive_args(&d.slides[0].directives[0]), "true");
    }

    #[test]
    fn directive_span_points_into_original_source() {
        let src = "<!-- oxlide-fx: fade -->\n\n# Slide";
        let d = deck(src);
        let dir = &d.slides[0].directives[0];
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
        let d = deck("- plain `code` text");
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
}
