use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as WidgetBlock, Borders, Paragraph, Widget, Wrap};

use crate::layout::layout;
use crate::parser::{Block, Cell, InlineSpan, ListItem, Slide};
use crate::render::composition::{compute_inner_area, is_hero_slide, render_hero};
use crate::render::theme::Theme;

pub fn render_slide(slide: &Slide, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if slide.cells.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let (inner, _chrome) = compute_inner_area(area, theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if is_hero_slide(slide) {
        render_hero(slide, inner, buf, theme);
        return;
    }
    let rects = layout(slide, inner);
    for (cell, rect) in slide.cells.iter().zip(rects) {
        render_cell(cell, rect, buf, theme);
    }
}

pub fn render_cell(cell: &Cell, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if cell.blocks.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }

    // Interleave a blank spacer row after any H1 that has a following block,
    // so the heading breathes above the body content.
    let mut entries: Vec<Option<usize>> = Vec::with_capacity(cell.blocks.len() * 2);
    for (i, block) in cell.blocks.iter().enumerate() {
        entries.push(Some(i));
        if matches!(block, Block::Heading { level: 1, .. }) && i + 1 < cell.blocks.len() {
            entries.push(None);
        }
    }

    let constraints: Vec<Constraint> = entries
        .iter()
        .map(|e| match e {
            Some(_) => Constraint::Min(1),
            None => Constraint::Length(1),
        })
        .collect();

    let rects = Layout::vertical(constraints).split(area);
    for (entry, rect) in entries.iter().zip(rects.iter()) {
        if let Some(i) = entry {
            render_block(&cell.blocks[*i], *rect, buf, theme);
        }
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
        Block::List { ordered, items, .. } => {
            render_list(*ordered, items, 0, area, buf, theme);
        }
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

fn render_list(
    ordered: bool,
    items: &[ListItem],
    depth: u16,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) -> u16 {
    if items.is_empty() || area.width == 0 || area.height == 0 {
        return 0;
    }
    let indent = depth.saturating_mul(2);
    let mut y_offset: u16 = 0;

    for (idx, item) in items.iter().enumerate() {
        if y_offset >= area.height {
            break;
        }
        let marker_text = if ordered {
            format!("{}. ", idx + 1)
        } else {
            "• ".to_string()
        };
        let marker_width = marker_text.chars().count() as u16;

        let mut first_block = true;
        for block in &item.blocks {
            if y_offset >= area.height {
                break;
            }
            match block {
                Block::List {
                    ordered: inner_ordered,
                    items: inner_items,
                    ..
                } => {
                    let nested = Rect {
                        x: area.x,
                        y: area.y + y_offset,
                        width: area.width,
                        height: area.height - y_offset,
                    };
                    let used = render_list(
                        *inner_ordered,
                        inner_items,
                        depth + 1,
                        nested,
                        buf,
                        theme,
                    );
                    y_offset = y_offset.saturating_add(used);
                }
                Block::Paragraph { spans, .. } | Block::Heading { spans, .. } => {
                    let ctx = ListItemLine {
                        spans,
                        is_first_block: first_block,
                        marker_text: &marker_text,
                        marker_width,
                        indent,
                        area,
                        y_offset,
                    };
                    render_list_item_line(&ctx, buf, theme);
                    y_offset = y_offset.saturating_add(1);
                }
                _ => {
                    y_offset = y_offset.saturating_add(1);
                }
            }
            first_block = false;
        }
    }
    y_offset
}

struct ListItemLine<'a> {
    spans: &'a [InlineSpan],
    is_first_block: bool,
    marker_text: &'a str,
    marker_width: u16,
    indent: u16,
    area: Rect,
    y_offset: u16,
}

fn render_list_item_line(ctx: &ListItemLine<'_>, buf: &mut Buffer, theme: &Theme) {
    let prefix_width = ctx.indent.saturating_add(ctx.marker_width);
    if prefix_width >= ctx.area.width {
        return;
    }
    let row_y = ctx.area.y + ctx.y_offset;

    if ctx.is_first_block {
        let marker_rect = Rect {
            x: ctx.area.x + ctx.indent,
            y: row_y,
            width: ctx.marker_width,
            height: 1,
        };
        Paragraph::new(ctx.marker_text.to_string())
            .style(theme.list_marker)
            .render(marker_rect, buf);
    }

    let content_rect = Rect {
        x: ctx.area.x + prefix_width,
        y: row_y,
        width: ctx.area.width - prefix_width,
        height: 1,
    };
    let line = inline_to_line(ctx.spans, theme);
    Paragraph::new(line)
        .style(theme.prose)
        .render(content_rect, buf);
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
    use crate::parser::{Block, Cell, InlineSpan, ListItem, Slide, SourceSpan};
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

    fn find_char_position(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            if let Some(x) = row.find(needle) {
                return Some((x as u16, y));
            }
        }
        None
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
        let (x, y) = find_char_position(&buf, "Hello").expect("heading rendered");
        let cell = &buf[(x, y)];
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
        // Hero slide ([H1, P]) renders heading centered with one blank row and
        // the subtitle below. Body must still appear after heading.
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

        let (_, title_y) = find_char_position(&buf, "Title").expect("title rendered");
        let (_, body_y) = find_char_position(&buf, "Body text").expect("body rendered");
        assert!(
            body_y > title_y,
            "body ({}) should render after heading ({})",
            body_y,
            title_y
        );
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

    fn list_item(blocks: Vec<Block>) -> ListItem {
        ListItem { blocks, span: span() }
    }

    fn simple_item(text: &str) -> ListItem {
        list_item(vec![paragraph(vec![InlineSpan::Text(text.into())])])
    }

    fn list(ordered: bool, items: Vec<ListItem>) -> Block {
        Block::List {
            ordered,
            items,
            span: span(),
        }
    }

    fn row_at(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn empty_list_does_not_panic() {
        let theme = Theme::default();
        let slide = slide_with_cell(vec![list(false, vec![])]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);
    }

    #[test]
    fn renders_unordered_list_with_bullet_markers() {
        let theme = Theme::default();
        let block = list(
            false,
            vec![
                simple_item("alpha"),
                simple_item("beta"),
                simple_item("gamma"),
            ],
        );
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let r0 = row_at(&buf, 0);
        let r1 = row_at(&buf, 1);
        let r2 = row_at(&buf, 2);
        assert!(r0.contains("• alpha"), "row 0: {:?}", r0);
        assert!(r1.contains("• beta"), "row 1: {:?}", r1);
        assert!(r2.contains("• gamma"), "row 2: {:?}", r2);

        let bullet_x = r0.find("•").expect("bullet on row 0") as u16;
        assert_eq!(buf[(bullet_x, 0)].symbol(), "•");
        assert_eq!(buf[(bullet_x, 0)].fg, theme.list_marker.fg.unwrap());
    }

    #[test]
    fn renders_ordered_list_with_numbered_markers() {
        let theme = Theme::default();
        let block = list(
            true,
            vec![
                simple_item("first"),
                simple_item("second"),
                simple_item("third"),
            ],
        );
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let r0 = row_at(&buf, 0);
        let r1 = row_at(&buf, 1);
        let r2 = row_at(&buf, 2);
        assert!(r0.contains("1. first"), "row 0: {:?}", r0);
        assert!(r1.contains("2. second"), "row 1: {:?}", r1);
        assert!(r2.contains("3. third"), "row 2: {:?}", r2);

        let marker_x = r0.find("1.").expect("marker on row 0") as u16;
        assert_eq!(buf[(marker_x, 0)].symbol(), "1");
        assert_eq!(buf[(marker_x, 0)].fg, theme.list_marker.fg.unwrap());
    }

    #[test]
    fn renders_nested_list_indented() {
        let theme = Theme::default();
        let inner = list(false, vec![simple_item("inner")]);
        let outer_item = list_item(vec![
            paragraph(vec![InlineSpan::Text("outer".into())]),
            inner,
        ]);
        let block = list(false, vec![outer_item]);
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let r0 = row_at(&buf, 0);
        let r1 = row_at(&buf, 1);
        assert!(r0.contains("• outer"), "row 0: {:?}", r0);
        assert!(r1.contains("  • inner"), "row 1: {:?}", r1);

        let outer_x = r0.find("•").expect("outer bullet on row 0") as u16;
        let inner_x = r1.find("•").expect("inner bullet on row 1") as u16;
        assert_eq!(buf[(outer_x, 0)].symbol(), "•");
        assert_eq!(buf[(inner_x, 1)].symbol(), "•");
        assert_eq!(inner_x, outer_x + 2, "inner bullet indented by 2 cells");
    }

    #[test]
    fn renders_list_items_with_inline_styles() {
        let theme = Theme::default();
        let spans = vec![
            InlineSpan::Strong(vec![InlineSpan::Text("bold".into())]),
            InlineSpan::Text(" ".into()),
            InlineSpan::Emphasis(vec![InlineSpan::Text("italic".into())]),
            InlineSpan::Text(" ".into()),
            InlineSpan::Code("code".into()),
            InlineSpan::Text(" ".into()),
            InlineSpan::Link {
                url: "https://example.com".into(),
                text: vec![InlineSpan::Text("link".into())],
            },
        ];
        let block = list(
            false,
            vec![list_item(vec![Block::Paragraph { spans, span: span() }])],
        );
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, area, &mut buf, &theme);

        let row = row_at(&buf, 0);
        assert!(row.contains("bold"), "row: {:?}", row);
        assert!(row.contains("italic"));
        assert!(row.contains("code"));
        assert!(row.contains("link"));

        let bold_x = row.find("bold").unwrap() as u16;
        assert!(buf[(bold_x, 0)].modifier.contains(Modifier::BOLD));

        let italic_x = row.find("italic").unwrap() as u16;
        assert!(buf[(italic_x, 0)].modifier.contains(Modifier::ITALIC));

        let code_x = row.find("code").unwrap() as u16;
        assert_eq!(buf[(code_x, 0)].fg, theme.code.fg.unwrap());

        let link_x = row.find("link").unwrap() as u16;
        assert_eq!(buf[(link_x, 0)].fg, theme.link.fg.unwrap());
        assert!(buf[(link_x, 0)].modifier.contains(Modifier::UNDERLINED));
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

        let (main_y, main_row) = find_row("fn main()").expect("fn main() row");
        let (_, println_row) = find_row("println!").expect("println row");
        let (_, close_row) = find_row("}").expect("closing brace row");

        let fn_x = main_row.find("fn main").expect("fn main x") as u16;
        let println_x = println_row.find("println").expect("println x") as u16;
        let close_x = close_row.find('}').expect("brace x") as u16;

        assert_eq!(
            println_x,
            fn_x + 4,
            "four-space indent preserved (fn at {}, println at {})",
            fn_x,
            println_x
        );
        assert_eq!(close_x, fn_x, "closing brace unindented");

        assert_eq!(buf[(fn_x, main_y)].fg, theme.code.fg.unwrap());
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
