use std::path::{Path, PathBuf};

use qrcode::QrCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as WidgetBlock, Borders, Paragraph, Widget, Wrap};
use ratatui_image::picker::Picker;
use ratatui_image::{Image, Resize};
use tui_qrcode::{Colors, QrCodeWidget, QuietZone, Scaling};

use crate::layout::layout;
use crate::parser::{Block, Cell, InlineSpan, ListItem, Slide};
use crate::render::composition::{compute_inner_area, is_hero_slide, render_hero};
use crate::render::text::truncate_to_width;
use crate::render::theme::{ChromeSpec, Theme};

/// Per-render context: the active theme, the deck file's parent directory
/// (used to resolve relative asset paths like `![alt](pics/foo.png)`), and
/// the terminal-detected `Picker` used to encode images. Built fresh each
/// frame by the caller — the picker itself is a one-time terminal capability
/// detection (not a dimension cache, so it does not violate the resize
/// invariant).
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub deck_dir: Option<&'a Path>,
    pub picker: Option<&'a Picker>,
}

impl<'a> RenderContext<'a> {
    /// Build a context with only a theme — used by tests and any caller that
    /// doesn't need image support. Image blocks fall back to the placeholder.
    pub fn from_theme(theme: &'a Theme) -> Self {
        Self {
            theme,
            deck_dir: None,
            picker: None,
        }
    }
}

pub fn render_slide(
    slide: &Slide,
    slide_idx: usize,
    total: usize,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let ctx = RenderContext::from_theme(theme);
    render_slide_with(slide, slide_idx, total, area, buf, &ctx);
}

/// Render a slide with full context (deck directory + picker). Used by
/// present-mode so image blocks can resolve relative paths and encode via
/// the terminal's preferred graphics protocol.
pub fn render_slide_with(
    slide: &Slide,
    slide_idx: usize,
    total: usize,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext<'_>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (inner, chrome_area) = compute_inner_area(area, ctx.theme);
    render_chrome(ctx.theme.chrome, chrome_area, slide_idx, total, ctx.theme, buf);
    if slide.cells.is_empty() || inner.width == 0 || inner.height == 0 {
        return;
    }
    if is_hero_slide(slide) {
        render_hero(slide, inner, buf, ctx.theme);
        return;
    }
    let rects = layout(slide, inner);
    for (cell, rect) in slide.cells.iter().zip(rects) {
        render_cell_with(cell, rect, buf, ctx);
    }
}

fn render_chrome(
    spec: ChromeSpec,
    chrome_area: Rect,
    slide_idx: usize,
    total: usize,
    theme: &Theme,
    buf: &mut Buffer,
) {
    if chrome_area.width == 0 || chrome_area.height == 0 {
        return;
    }
    match spec {
        ChromeSpec::None => {}
        ChromeSpec::BottomRule => {
            let rule_rect = Rect {
                x: chrome_area.x,
                y: chrome_area.y,
                width: chrome_area.width,
                height: 1,
            };
            let rule = "─".repeat(chrome_area.width as usize);
            Paragraph::new(rule)
                .style(theme.chrome_dim)
                .render(rule_rect, buf);

            if chrome_area.height >= 2 {
                let counter_rect = Rect {
                    x: chrome_area.x,
                    y: chrome_area.y + 1,
                    width: chrome_area.width,
                    height: 1,
                };
                // ratatui's Alignment::Center is unicode-width aware — no
                // manual cell math needed regardless of heading content or
                // terminal width.
                let counter = format!("{} / {}", slide_idx + 1, total);
                Paragraph::new(counter)
                    .style(theme.chrome_dim)
                    .alignment(Alignment::Center)
                    .render(counter_rect, buf);
            }
        }
    }
}

pub fn render_cell(cell: &Cell, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let ctx = RenderContext::from_theme(theme);
    render_cell_with(cell, area, buf, &ctx);
}

fn render_cell_with(cell: &Cell, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_>) {
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
            render_block(&cell.blocks[*i], *rect, buf, ctx);
        }
    }
}

fn render_block(block: &Block, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_>) {
    let theme = ctx.theme;
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
            render_image(src, alt, area, buf, ctx);
        }
        Block::Qr { url, .. } => {
            render_qr(url, area, buf, theme);
        }
    }
}

fn render_code(_lang: &Option<String>, source: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = WidgetBlock::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(theme.chrome_dim);
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    Paragraph::new(source.to_string())
        .style(theme.code)
        .render(inner, buf);
}

fn render_image(src: &str, alt: &str, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if try_render_image(src, area, buf, ctx).is_none() {
        render_image_placeholder(src, alt, area, buf, ctx.theme);
    }
}

/// Resolve a markdown image `src` to a filesystem path. Absolute paths are
/// honored as-is; relative paths are joined onto the deck file's parent
/// directory if known. With no deck dir (e.g. tests), the relative path is
/// returned unchanged — `image::ImageReader::open` will then fail and the
/// caller will fall back to the placeholder.
pub(crate) fn resolve_image_path(src: &str, deck_dir: Option<&Path>) -> PathBuf {
    let p = Path::new(src);
    if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(dir) = deck_dir {
        dir.join(p)
    } else {
        p.to_path_buf()
    }
}

/// Try to decode and render the image. Returns `None` on any failure —
/// missing file, decode error, no picker available, encoder error — so the
/// caller can fall back to the textual placeholder.
fn try_render_image(
    src: &str,
    area: Rect,
    buf: &mut Buffer,
    ctx: &RenderContext<'_>,
) -> Option<()> {
    let picker = ctx.picker?;
    let path = resolve_image_path(src, ctx.deck_dir);

    let dyn_img = image::ImageReader::open(&path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;

    let protocol = picker.new_protocol(dyn_img, area, Resize::Fit(None)).ok()?;
    Image::new(&protocol).render(area, buf);
    Some(())
}

fn render_image_placeholder(src: &str, alt: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
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

/// Extra quiet-zone modules added around the raw QR (4 on each side per the
/// spec). tui-qrcode's QuietZone::Enabled adds the same margin.
const QR_QUIET_MODULES: u16 = 8;

fn render_qr(url: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let trimmed = url.trim();
    if trimmed.is_empty() {
        render_qr_error(url, "empty QR URL", area, buf, theme);
        return;
    }

    // Reserve 1 row at the bottom for the caption; the QR claims the rest.
    let caption_height: u16 = if area.height >= 2 { 1 } else { 0 };
    let qr_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(caption_height),
    };

    let qr = match QrCode::new(trimmed.as_bytes()) {
        Ok(q) => q,
        Err(_) => {
            render_qr_error(trimmed, "could not encode URL", area, buf, theme);
            return;
        }
    };

    // Minimum cell budget at one-cell-per-module scale. Half-block glyphs
    // pack two modules vertically, so height = ceil(modules / 2).
    let modules = qr.width() as u16 + QR_QUIET_MODULES;
    let min_width = modules;
    let min_height = modules.div_ceil(2);
    if qr_area.width < min_width || qr_area.height < min_height {
        render_qr_error(trimmed, "QR cell too small", area, buf, theme);
        return;
    }

    let widget = QrCodeWidget::new(qr)
        .colors(Colors::Normal)
        .quiet_zone(QuietZone::Enabled)
        .scaling(Scaling::Min);

    // Render at minimum scale centered inside qr_area; tui-qrcode's Min
    // scaling will expand to fill the provided Rect, so we hand it exactly
    // the minimum footprint to keep the QR square-ish and consistent.
    let pad_x = (qr_area.width - min_width) / 2;
    let pad_y = (qr_area.height - min_height) / 2;
    let centered = Rect {
        x: qr_area.x + pad_x,
        y: qr_area.y + pad_y,
        width: min_width,
        height: min_height,
    };
    widget.render(centered, buf);

    if caption_height == 0 {
        return;
    }
    let caption_rect = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let caption = truncate_to_width(trimmed, area.width as usize);
    let muted = theme.prose.add_modifier(ratatui::style::Modifier::DIM);
    Paragraph::new(caption)
        .style(muted)
        .alignment(Alignment::Center)
        .render(caption_rect, buf);
}

fn render_qr_error(raw: &str, reason: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = WidgetBlock::default().borders(Borders::ALL).title(" qr ");
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let muted = theme.prose.add_modifier(ratatui::style::Modifier::DIM);
    let reason_line = truncate_to_width(reason, inner.width as usize);
    let mut lines: Vec<Line<'static>> = vec![Line::styled(reason_line, theme.image_placeholder)];
    if !raw.is_empty() {
        let detail = truncate_to_width(raw, inner.width as usize);
        lines.push(Line::styled(detail, muted));
    }
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
                    let used =
                        render_list(*inner_ordered, inner_items, depth + 1, nested, buf, theme);
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

fn collect_spans(spans: &[InlineSpan], theme: &Theme, base: Style, out: &mut Vec<Span<'static>>) {
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
        Block::Paragraph {
            spans,
            span: span(),
        }
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
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![heading(1, "Hello")]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(
            text.contains("Hello"),
            "buffer should contain heading text; got {:?}",
            text
        );

        // Paper-white headings carry weight (bold), not color — fg falls
        // through to the terminal default (Color::Reset).
        let (x, y) = find_char_position(&buf, "Hello").expect("heading rendered");
        let cell = &buf[(x, y)];
        assert_eq!(cell.symbol(), "H");
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn renders_paragraph_text_with_inline_styles() {
        let theme = Theme::paper_white();
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
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
        // Hero slide ([H1, P]) renders heading centered with one blank row and
        // the subtitle below. Body must still appear after heading.
        let slide = slide_with_cell(vec![
            heading(1, "Title"),
            paragraph(vec![InlineSpan::Text("Body text".into())]),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
        let spans = vec![InlineSpan::Link {
            url: "https://example.com".into(),
            text: vec![InlineSpan::Text("click".into())],
        }];
        let slide = slide_with_cell(vec![paragraph(spans)]);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let row: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        let link_x = row.find("click").expect("link text on first row") as u16;
        let cell = &buf[(link_x, 0)];
        assert_eq!(cell.fg, theme.link.fg.unwrap());
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn empty_cell_does_not_panic() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);
    }

    #[test]
    fn zero_size_area_does_not_panic() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![heading(1, "Hello")]);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);
    }

    fn list_item(blocks: Vec<Block>) -> ListItem {
        ListItem {
            blocks,
            span: span(),
        }
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
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![list(false, vec![])]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);
    }

    #[test]
    fn renders_unordered_list_with_bullet_markers() {
        let theme = Theme::paper_white();
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
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
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
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
        let inner = list(false, vec![simple_item("inner")]);
        let outer_item = list_item(vec![
            paragraph(vec![InlineSpan::Text("outer".into())]),
            inner,
        ]);
        let block = list(false, vec![outer_item]);
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
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
            vec![list_item(vec![Block::Paragraph {
                spans,
                span: span(),
            }])],
        );
        let slide = slide_with_cell(vec![block]);
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
        let src = "fn main() {\n    println!(\"hi\");\n}";
        let slide = slide_with_cell(vec![code_block(src)]);
        // Need top+bottom borders + 3 code rows + 2 chrome rows = 7.
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let find_row = |needle: &str| -> Option<(u16, String)> {
            (0..buf.area.height).find_map(|y| {
                let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                if row.contains(needle) {
                    Some((y, row))
                } else {
                    None
                }
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
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![code_block("x")]);
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        let bottom: String = (0..buf.area.width)
            .map(|x| buf[(x, buf.area.height - 1)].symbol())
            .collect();
        assert!(top.contains("─"), "top border missing; got {:?}", top);
        assert!(
            bottom.contains("─"),
            "bottom border missing; got {:?}",
            bottom
        );
    }

    #[test]
    fn code_block_clips_long_lines_without_wrapping() {
        let theme = Theme::paper_white();
        let long = "a".repeat(80);
        let slide = slide_with_cell(vec![code_block(&long)]);
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let rows: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let a_rows = rows.iter().filter(|r| r.contains("aaaa")).count();
        assert_eq!(a_rows, 1, "long line should not wrap; rows={:?}", rows);
    }

    #[test]
    fn empty_code_block_does_not_panic() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![code_block("")]);
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let top: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(
            top.contains("─"),
            "frame should render even with empty source"
        );
    }

    #[test]
    fn renders_image_block_with_alt_and_src() {
        let theme = Theme::paper_white();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: "a cat".into(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

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
        let theme = Theme::paper_white();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: String::new(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("cat.png"), "src missing; got {:?}", text);
        assert!(text.contains("image"), "title missing; got {:?}", text);
    }

    #[test]
    fn narrow_image_area_does_not_panic() {
        let theme = Theme::paper_white();
        let image = Block::Image {
            src: "cat.png".into(),
            alt: "a cat".into(),
            meta: None,
            span: span(),
        };
        let slide = slide_with_cell(vec![image]);
        let area = Rect::new(0, 0, 4, 3);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);
    }

    // ----- image path resolution + missing-asset fallback -----

    #[test]
    fn resolve_image_path_honors_absolute_paths() {
        // Absolute paths must not be re-rooted under the deck dir.
        let deck_dir = Path::new("/tmp/deck");
        let abs = if cfg!(windows) { r"C:\img\cat.png" } else { "/img/cat.png" };
        let resolved = resolve_image_path(abs, Some(deck_dir));
        assert_eq!(resolved, Path::new(abs));
    }

    #[test]
    fn resolve_image_path_joins_relative_to_deck_dir() {
        let deck_dir = Path::new("/tmp/deck");
        let resolved = resolve_image_path("pics/cat.png", Some(deck_dir));
        assert_eq!(resolved, Path::new("/tmp/deck/pics/cat.png"));
    }

    #[test]
    fn resolve_image_path_returns_relative_unchanged_when_no_deck_dir() {
        // Without a deck dir, we can't resolve — let the open() call fail and
        // fall through to the placeholder.
        let resolved = resolve_image_path("pics/cat.png", None);
        assert_eq!(resolved, Path::new("pics/cat.png"));
    }

    #[test]
    fn resolve_image_path_handles_emoji_filename() {
        // Per oxlide rendering invariants: multi-byte characters in any path
        // segment must round-trip through Path without corruption.
        let deck_dir = Path::new("/tmp/deck");
        let resolved = resolve_image_path("party-🎉.png", Some(deck_dir));
        assert_eq!(resolved, Path::new("/tmp/deck/party-🎉.png"));
    }

    #[test]
    fn resolve_image_path_handles_cjk_filename() {
        let deck_dir = Path::new("/tmp/deck");
        let resolved = resolve_image_path("写真/猫.png", Some(deck_dir));
        assert_eq!(resolved, Path::new("/tmp/deck/写真/猫.png"));
    }

    #[test]
    fn missing_image_with_picker_renders_placeholder_not_panic() {
        // Even when a Picker is available, a missing source file must fall
        // through to the textual placeholder — never a panic, never a crash.
        let theme = Theme::paper_white();
        let picker = Picker::halfblocks();
        let ctx = RenderContext {
            theme: &theme,
            deck_dir: Some(Path::new("/nonexistent/deck/dir")),
            picker: Some(&picker),
        };
        let slide = slide_with_cell(vec![Block::Image {
            src: "no-such-image.png".into(),
            alt: "missing".into(),
            meta: None,
            span: span(),
        }]);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        render_slide_with(&slide, 0, 1, area, &mut buf, &ctx);

        let text = buffer_text(&buf);
        assert!(
            text.contains("missing"),
            "alt text should appear in placeholder; got {:?}",
            text
        );
        assert!(
            text.contains("no-such-image.png"),
            "src should appear in placeholder; got {:?}",
            text
        );
    }

    #[test]
    fn missing_image_resize_rerenders_cleanly_at_different_widths() {
        // Per oxlide rendering invariants: every render takes Rect fresh, no
        // cached state. Render the same image slide at several sizes — must
        // never panic, must always show the placeholder (file doesn't exist).
        let theme = Theme::paper_white();
        let picker = Picker::halfblocks();
        let ctx = RenderContext {
            theme: &theme,
            deck_dir: Some(Path::new("/nonexistent/deck/dir")),
            picker: Some(&picker),
        };
        let slide = slide_with_cell(vec![Block::Image {
            src: "no-such-image.png".into(),
            alt: "missing".into(),
            meta: None,
            span: span(),
        }]);

        for (w, h) in [(60u16, 24u16), (120, 40), (40, 16), (200, 60), (8, 4)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            render_slide_with(&slide, 0, 1, area, &mut buf, &ctx);
            // Either the placeholder rendered (alt appears) or the area was
            // too narrow to fit any text. Never a panic, never a crash.
            let _ = buffer_text(&buf);
        }
    }

    #[test]
    fn inline_image_span_renders_placeholder() {
        let theme = Theme::paper_white();
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
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let row: String = (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row.contains("[img: cat]"), "row was {:?}", row);
    }

    fn qr_block(url: &str) -> Block {
        Block::Qr {
            url: url.into(),
            span: span(),
        }
    }

    fn has_block_char(buf: &Buffer) -> bool {
        // tui-qrcode paints QR modules with half-block glyphs ('▀', '▄',
        // '█', ' '). A scannable QR must have painted dark cells.
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let s = buf[(x, y)].symbol();
                if s == "▀" || s == "▄" || s == "█" {
                    return true;
                }
            }
        }
        false
    }

    fn find_row_substring(buf: &Buffer, needle: &str) -> Option<(u16, String)> {
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            if row.contains(needle) {
                return Some((y, row));
            }
        }
        None
    }

    #[test]
    fn renders_qr_block_with_url_caption() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("https://github.com/cmoel/oxlide")]);
        // Generous canvas so the QR comfortably fits.
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        assert!(
            has_block_char(&buf),
            "expected QR block glyphs to appear, got:\n{}",
            buffer_text(&buf)
        );
        let (_, row) = find_row_substring(&buf, "https://github.com/cmoel/oxlide")
            .expect("caption rendered verbatim");
        // Caption should not appear wrapped.
        assert!(
            row.contains("https://github.com/cmoel/oxlide"),
            "caption row: {:?}",
            row
        );
    }

    #[test]
    fn qr_caption_truncates_to_cell_width_with_ellipsis() {
        let theme = Theme::paper_white();
        let url = "https://github.com/cmoel/oxlide/issues/42/very/long";
        let slide = slide_with_cell(vec![qr_block(url)]);
        // 16 cells wide: caption "https://github.c…" or similar — truncated.
        let area = Rect::new(0, 0, 16, 16);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        // At this width, the QR itself likely fails the min-size check and
        // we get the "QR cell too small" error card. The full URL must not
        // appear verbatim — truncation is the invariant we assert here.
        let text = buffer_text(&buf);
        assert!(
            !text.contains(url),
            "full URL should not fit verbatim at width 16; got:\n{}",
            text
        );
    }

    #[test]
    fn qr_empty_url_renders_error_card() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("")]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(
            text.contains("empty QR URL"),
            "expected error card, got:\n{}",
            text
        );
        assert!(
            !has_block_char(&buf),
            "must not emit QR modules for empty URL"
        );
    }

    #[test]
    fn qr_whitespace_only_url_renders_error_card() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("   ")]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(text.contains("empty QR URL"), "got:\n{}", text);
    }

    #[test]
    fn qr_too_small_renders_error_card() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("https://x.com")]);
        // 30 cols wide — narrow enough that the minimum QR footprint (37×19)
        // cannot fit, but wide enough to print the full error message.
        let area = Rect::new(0, 0, 30, 10);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        assert!(
            text.contains("QR cell too small"),
            "expected size error, got:\n{}",
            text
        );
    }

    #[test]
    fn qr_zero_sized_area_does_not_panic() {
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("https://x.com")]);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);
    }

    #[test]
    fn qr_unicode_url_does_not_panic_and_truncates_cleanly() {
        // Rendering invariant (per oxlide-rendering-invariants): Unicode URL
        // must not panic, and caption truncation must stay width-correct.
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("https://例え.jp/path/with/中文")]);
        let area = Rect::new(0, 0, 80, 30);
        let mut buf = Buffer::empty(area);
        render_slide(&slide, 0, 1, area, &mut buf, &theme);

        let text = buffer_text(&buf);
        // Some recognizable substring of the URL should have made it through.
        assert!(
            text.contains("https://") || text.contains("例え"),
            "expected URL fragment in output; got:\n{}",
            text
        );
    }

    #[test]
    fn qr_resize_rerenders_cleanly_at_different_widths() {
        // Rendering invariant: every frame takes Rect fresh — no cached state.
        // Render the same slide at several sizes, no panics, each renders.
        let theme = Theme::paper_white();
        let slide = slide_with_cell(vec![qr_block("https://github.com/cmoel/oxlide")]);

        for (w, h) in [(60u16, 24u16), (120, 40), (40, 16), (200, 60), (80, 30)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            render_slide(&slide, 0, 1, area, &mut buf, &theme);
            let text = buffer_text(&buf);
            // Either a scannable QR (w,h big enough) or an error card — never
            // nothing, never a panic.
            assert!(
                has_block_char(&buf) || text.contains("QR cell too small"),
                "size {}x{} produced neither QR nor error:\n{}",
                w,
                h,
                text
            );
        }
    }
}
