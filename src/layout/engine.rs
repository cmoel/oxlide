use crate::parser::Slide;
use ratatui::layout::Rect;

pub fn layout(slide: &Slide, area: Rect) -> Vec<Rect> {
    match slide.cells.len() {
        0 => vec![],
        1 => vec![area],
        _ => todo!("multi-cell layouts — see oxlide-y2n.2"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Block, Cell, InlineSpan, Slide, SourceSpan};

    fn span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    fn paragraph_cell() -> Cell {
        Cell {
            blocks: vec![Block::Paragraph {
                spans: vec![InlineSpan::Text("hello".into())],
                span: span(),
            }],
            directives: vec![],
            span: span(),
        }
    }

    fn slide_with_cells(cells: Vec<Cell>) -> Slide {
        Slide {
            cells,
            notes: vec![],
            directives: vec![],
            span: span(),
        }
    }

    #[test]
    fn one_cell_returns_whole_area() {
        let slide = slide_with_cells(vec![paragraph_cell()]);
        let area = Rect::new(0, 0, 80, 24);
        let rects = layout(&slide, area);
        assert_eq!(rects, vec![area]);
    }

    #[test]
    fn zero_cells_returns_empty_vec() {
        let slide = slide_with_cells(vec![]);
        let area = Rect::new(0, 0, 80, 24);
        let rects = layout(&slide, area);
        assert!(rects.is_empty());
    }

    #[test]
    fn one_cell_with_zero_size_area_still_returns_area() {
        let slide = slide_with_cells(vec![paragraph_cell()]);
        let area = Rect::new(0, 0, 0, 0);
        let rects = layout(&slide, area);
        assert_eq!(rects, vec![area]);
    }
}
