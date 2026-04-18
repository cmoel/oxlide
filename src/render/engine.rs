use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::layout::layout;
use crate::parser::{Block, Cell, InlineSpan, Slide};
use crate::render::theme::Theme;

pub fn render_slide(slide: &Slide, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rects = layout(slide, area);
    for (cell, rect) in slide.cells.iter().zip(rects) {
        render_cell(cell, rect, buf, theme);
    }
}

pub fn render_cell(cell: &Cell, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if cell.blocks.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let constraints: Vec<Constraint> = cell.blocks.iter().map(|_| Constraint::Min(1)).collect();
    let rects = Layout::vertical(constraints).split(area);
    for (block, rect) in cell.blocks.iter().zip(rects.iter()) {
        render_block(block, *rect, buf, theme);
    }
}

fn render_block(block: &Block, area: Rect, buf: &mut Buffer, theme: &Theme) {
    match block {
        Block::Heading { level, spans, .. } => {
            let idx = (level.saturating_sub(1) as usize).min(5);
            let style = theme.heading[idx];
            let line = inline_to_line(spans, theme);
            Paragraph::new(line).style(style).render(area, buf);
        }
        Block::Paragraph { spans, .. } => {
            let line = inline_to_line(spans, theme);
            Paragraph::new(line)
                .style(theme.prose)
                .wrap(Wrap { trim: true })
                .render(area, buf);
        }
        Block::List { .. } | Block::CodeBlock { .. } | Block::Image { .. } => {}
    }
}

pub fn inline_to_line(spans: &[InlineSpan], theme: &Theme) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    collect_spans(spans, theme, Style::default(), &mut out);
    Line::from(out)
}

fn collect_spans(
    spans: &[InlineSpan],
    theme: &Theme,
    base: Style,
    out: &mut Vec<Span<'static>>,
) {
    for span in spans {
        match span {
            InlineSpan::Text(text) => {
                out.push(Span::styled(text.clone(), base));
            }
            InlineSpan::Strong(children) => {
                let style = base.add_modifier(ratatui::style::Modifier::BOLD);
                collect_spans(children, theme, style, out);
            }
            InlineSpan::Emphasis(children) => {
                let style = base.add_modifier(ratatui::style::Modifier::ITALIC);
                collect_spans(children, theme, style, out);
            }
            InlineSpan::Code(text) => {
                out.push(Span::styled(text.clone(), theme.code));
            }
            InlineSpan::Link { text, .. } => {
                collect_spans(text, theme, theme.link, out);
            }
            InlineSpan::Image { alt, .. } => {
                out.push(Span::styled(
                    format!("[img: {}]", alt),
                    theme.image_placeholder,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Block, Cell, InlineSpan, Slide, SourceSpan};
    use ratatui::style::Modifier;

    fn span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    fn heading(level: u8, text: &str) -> Block {
        Block::Heading {
            level,
            spans: vec![InlineSpan::Text(text.into())],
            span: span(),
        }
    }

    fn paragraph(spans: Vec<InlineSpan>) -> Block {
        Block::Paragraph { spans, span: span() }
    }

    fn slide_with_cell(blocks: Vec<Block>) -> Slide {
        Slide {
            cells: vec![Cell {
                blocks,
                directives: vec![],
                span: span(),
            }],
            notes: vec![],
            directives: vec![],
            span: span(),
        }
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

    #[test]
    fn renders_heading_text_with_heading_style() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![heading(1, "Hello")]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("Hello"), "buffer should contain heading text; got {:?}", text);

        let expected = theme.heading[0];
        let cell = &buf[(0, 0)];
        assert_eq!(cell.symbol(), "H");
        assert_eq!(cell.fg, expected.fg.unwrap());
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn renders_paragraph_text_with_inline_styles() {
        let theme = Theme::default();
        let spans = vec![
            InlineSpan::Text("plain ".into()),
            InlineSpan::Strong(vec![InlineSpan::Text("bold".into())]),
            InlineSpan::Text(" ".into()),
            InlineSpan::Emphasis(vec![InlineSpan::Text("italic".into())]),
            InlineSpan::Text(" ".into()),
            InlineSpan::Code("code".into()),
        ];
        let slide = slide_with_cell(vec![paragraph(spans)]);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("plain"));
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(text.contains("code"));

        let full_line: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        let bold_x = full_line.find("bold").expect("bold text on first row") as u16;
        assert!(buf[(bold_x, 0)].modifier.contains(Modifier::BOLD));

        let italic_x = full_line.find("italic").expect("italic text on first row") as u16;
        assert!(buf[(italic_x, 0)].modifier.contains(Modifier::ITALIC));

        let code_x = full_line.find("code").expect("code text on first row") as u16;
        assert_eq!(buf[(code_x, 0)].fg, theme.code.fg.unwrap());
    }

    #[test]
    fn renders_heading_and_paragraph_stacked() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![
            heading(1, "Title"),
            paragraph(vec![InlineSpan::Text("Body text".into())]),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("Title"));
        assert!(text.contains("Body text"));

        let title_line: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(title_line.contains("Title"));

        let mut body_row = None;
        for y in 1..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            if row.contains("Body text") {
                body_row = Some(y);
                break;
            }
        }
        assert!(body_row.is_some(), "paragraph should render below heading");
    }

    #[test]
    fn renders_link_with_link_style() {
        let theme = Theme::default();
        let spans = vec![InlineSpan::Link {
            url: "https://example.com".into(),
            text: vec![InlineSpan::Text("click".into())],
        }];
        let slide = slide_with_cell(vec![paragraph(spans)]);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let row: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        let link_x = row.find("click").expect("link text on first row") as u16;
        let cell = &buf[(link_x, 0)];
        assert_eq!(cell.fg, theme.link.fg.unwrap());
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn empty_cell_does_not_panic() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);
    }

    #[test]
    fn zero_size_area_does_not_panic() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![heading(1, "Hello")]);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);
    }

    #[test]
    fn list_codeblock_image_blocks_are_noops() {
        let theme = Theme::default();
        let list = Block::List {
            ordered: false,
            items: vec![],
            span: span(),
        };
        let code = Block::CodeBlock {
            lang: None,
            source: "fn main() {}".into(),
            span: span(),
        };
        let image = Block::Image {
            src: "x.png".into(),
            alt: "x".into(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![list, code, image]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(!text.contains("fn main"));
        assert!(!text.contains("x.png"));
    }

    #[test]
    fn inline_image_span_renders_placeholder() {
        let theme = Theme::default();
        let spans = vec![
            InlineSpan::Text("before ".into()),
            InlineSpan::Image {
                src: "pic.png".into(),
                alt: "cat".into(),
            },
            InlineSpan::Text(" after".into()),
        ];
        let slide = slide_with_cell(vec![paragraph(spans)]);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let row: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row.contains("[img: cat]"), "row was {:?}", row);
    }
}
