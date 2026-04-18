use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as WidgetBlock, Borders, Paragraph, Widget, Wrap};

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
        Block::List { .. } => {}
        Block::CodeBlock { lang, source, .. } => {
            render_code(lang, source, area, buf, theme);
        }
        Block::Image { src, alt, .. } => {
            render_image(src, alt, area, buf, theme);
        }
    }
}

fn render_code(_lang: &Option<String>, source: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let border_style = theme.prose.add_modifier(ratatui::style::Modifier::DIM);
    let block = WidgetBlock::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(border_style);
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    Paragraph::new(source.to_string())
        .style(theme.code)
        .render(inner, buf);
}

fn render_image(src: &str, alt: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = WidgetBlock::default()
        .borders(Borders::ALL)
        .title(" image ");
    let inner = block.inner(area);
    block.render(area, buf);

    let muted = theme.prose.add_modifier(ratatui::style::Modifier::DIM);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if !alt.is_empty() {
        lines.push(Line::styled(alt.to_string(), theme.image_placeholder));
    }
    lines.push(Line::styled(src.to_string(), muted));

    Paragraph::new(lines).render(inner, buf);
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
    fn list_block_is_noop() {
        let theme = Theme::default();
        let list = Block::List {
            ordered: false,
            items: vec![],
            span: span(),
        };
        let slide = slide_with_cell(vec![list]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);
    }

    fn code_block(source: &str) -> Block {
        Block::CodeBlock {
            lang: None,
            source: source.into(),
            span: span(),
        }
    }

    #[test]
    fn renders_code_block_preserving_whitespace() {
        let theme = Theme::default();
        let src = "fn main() {\n    println!(\"hi\");\n}";
        let slide = slide_with_cell(vec![code_block(src)]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let find_row = |needle: &str| -> Option<(u16, String)> {
            (0..buf.area.height).find_map(|y| {
                let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                if row.contains(needle) { Some((y, row)) } else { None }
            })
        };

        let (_, main_row) = find_row("fn main()").expect("fn main() row");
        assert!(main_row.starts_with("fn main()"), "no leading indent on line 1; got {:?}", main_row);

        let (_, println_row) = find_row("println!").expect("println row");
        assert!(
            println_row.starts_with("    println!"),
            "indentation not preserved; got {:?}",
            println_row
        );

        let (_, close_row) = find_row("}").expect("closing brace row");
        assert!(close_row.starts_with("}"), "got {:?}", close_row);

        let code_x = main_row.find("fn").unwrap() as u16;
        let code_y = (0..buf.area.height)
            .find(|&y| {
                let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                row.contains("fn main()")
            })
            .unwrap();
        assert_eq!(buf[(code_x, code_y)].fg, theme.code.fg.unwrap());
    }

    #[test]
    fn code_block_has_visible_frame() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![code_block("x")]);
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        let bottom: String = (0..buf.area.width)
            .map(|x| buf[(x, buf.area.height - 1)].symbol())
            .collect();
        assert!(top.contains("─"), "top border missing; got {:?}", top);
        assert!(bottom.contains("─"), "bottom border missing; got {:?}", bottom);
    }

    #[test]
    fn code_block_clips_long_lines_without_wrapping() {
        let theme = Theme::default();
        let long = "a".repeat(80);
        let slide = slide_with_cell(vec![code_block(&long)]);
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let rows: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let a_rows = rows.iter().filter(|r| r.contains("aaaa")).count();
        assert_eq!(a_rows, 1, "long line should not wrap; rows={:?}", rows);
    }

    #[test]
    fn empty_code_block_does_not_panic() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![code_block("")]);
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(top.contains("─"), "frame should render even with empty source");
    }

    #[test]
    fn renders_image_block_with_alt_and_src() {
        let theme = Theme::default();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: "a cat".into(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("a cat"), "alt text missing; got {:?}", text);
        assert!(text.contains("cat.png"), "src missing; got {:?}", text);
        assert!(text.contains("image"), "title missing; got {:?}", text);

        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(
            top.contains("─") || top.contains("┌"),
            "top border missing; got {:?}",
            top
        );

        let alt_row = (0..buf.area.height).find(|&y| {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            row.contains("a cat")
        });
        let alt_y = alt_row.expect("alt row present");
        let alt_row_str: String = (0..buf.area.width)
            .map(|x| buf[(x, alt_y)].symbol())
            .collect();
        let alt_x = alt_row_str.find("a cat").unwrap() as u16;
        assert_eq!(
            buf[(alt_x, alt_y)].fg,
            theme.image_placeholder.fg.unwrap(),
            "alt text should use image_placeholder style"
        );
    }

    #[test]
    fn renders_image_block_with_empty_alt() {
        let theme = Theme::default();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: String::new(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("cat.png"), "src missing; got {:?}", text);
        assert!(text.contains("image"), "title missing; got {:?}", text);
    }

    #[test]
    fn narrow_image_area_does_not_panic() {
        let theme = Theme::default();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: "a cat".into(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 4, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);
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
