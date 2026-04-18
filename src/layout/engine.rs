use crate::parser::Slide;
use ratatui::layout::{Constraint, Layout, Rect};

/// Minimum width for any column in a multi-cell layout. When a proposed column
/// would be narrower than this, the whole slide falls back to a vertical stack.
pub(crate) const MIN_COL_WIDTH: u16 = 40;

pub fn layout(slide: &Slide, area: Rect) -> Vec<Rect> {
    let n = slide.cells.len();
    match n {
        0 => return vec![],
        1 => return vec![area],
        _ => {}
    }

    let rows = row_distribution(n);
    let max_cols = *rows.iter().max().expect("multi-cell slide has rows") as u16;
    if max_cols > 0 && area.width / max_cols < MIN_COL_WIDTH {
        return vertical_stack(n, area);
    }

    let row_count = rows.len() as u32;
    let row_constraints: Vec<Constraint> = (0..row_count)
        .map(|_| Constraint::Ratio(1, row_count))
        .collect();
    let row_rects = Layout::vertical(row_constraints).split(area);

    let mut result = Vec::with_capacity(n);
    for (row_idx, &cols_in_row) in rows.iter().enumerate() {
        let cols = cols_in_row as u32;
        let col_constraints: Vec<Constraint> =
            (0..cols).map(|_| Constraint::Ratio(1, cols)).collect();
        let col_rects = Layout::horizontal(col_constraints).split(row_rects[row_idx]);
        for r in col_rects.iter() {
            result.push(*r);
        }
    }
    result
}

fn vertical_stack(n: usize, area: Rect) -> Vec<Rect> {
    let row_count = n as u32;
    let row_constraints: Vec<Constraint> = (0..row_count)
        .map(|_| Constraint::Ratio(1, row_count))
        .collect();
    let rects = Layout::vertical(row_constraints).split(area);
    rects.iter().copied().collect()
}

fn row_distribution(n: usize) -> Vec<usize> {
    match n {
        0 => vec![],
        1 => vec![1],
        2 => vec![2],
        3 => vec![3],
        4 => vec![2, 2],
        _ => {
            let mut rows = Vec::new();
            let mut remaining = n;
            while remaining >= 3 {
                rows.push(3);
                remaining -= 3;
            }
            if remaining > 0 {
                rows.push(remaining);
            }
            // Orphan avoidance: a trailing row of 1 becomes 2+2 with the prior row.
            if *rows.last().unwrap() == 1 {
                let last = rows.len() - 1;
                rows[last] = 2;
                rows[last - 1] = 2;
            }
            rows
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Block, Cell, InlineSpan, Slide, SourceSpan};
    use std::collections::BTreeSet;

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

    fn slide_with_n(n: usize) -> Slide {
        slide_with_cells((0..n).map(|_| paragraph_cell()).collect())
    }

    fn distinct_y_rows(rects: &[Rect]) -> usize {
        rects.iter().map(|r| r.y).collect::<BTreeSet<_>>().len()
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

    #[test]
    fn two_cells_split_horizontally() {
        let rects = layout(&slide_with_n(2), Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 20));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 20));
    }

    #[test]
    fn three_cells_split_into_three_columns() {
        let rects = layout(&slide_with_n(3), Rect::new(0, 0, 150, 20));
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 20));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 20));
        assert_eq!(rects[2], Rect::new(100, 0, 50, 20));
    }

    #[test]
    fn four_cells_form_two_by_two_grid() {
        let rects = layout(&slide_with_n(4), Rect::new(0, 0, 100, 100));
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 50));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 50));
        assert_eq!(rects[2], Rect::new(0, 50, 50, 50));
        assert_eq!(rects[3], Rect::new(50, 50, 50, 50));
    }

    #[test]
    fn five_cells_split_three_plus_two() {
        let rects = layout(&slide_with_n(5), Rect::new(0, 0, 120, 40));
        assert_eq!(rects.len(), 5);
        assert_eq!(distinct_y_rows(&rects), 2);
        // Row 0: indexes 0..=2 share y=0
        assert!(rects[0..3].iter().all(|r| r.y == 0));
        // Row 1: indexes 3..=4 share y=20
        assert!(rects[3..5].iter().all(|r| r.y == 20));
    }

    #[test]
    fn six_cells_split_three_plus_three() {
        let rects = layout(&slide_with_n(6), Rect::new(0, 0, 120, 40));
        assert_eq!(rects.len(), 6);
        assert_eq!(distinct_y_rows(&rects), 2);
        assert!(rects[0..3].iter().all(|r| r.y == 0));
        assert!(rects[3..6].iter().all(|r| r.y == 20));
    }

    #[test]
    fn seven_cells_rebalance_to_three_two_two() {
        let rects = layout(&slide_with_n(7), Rect::new(0, 0, 120, 60));
        assert_eq!(rects.len(), 7);
        assert_eq!(distinct_y_rows(&rects), 3);
        assert!(rects[0..3].iter().all(|r| r.y == 0));
        assert!(rects[3..5].iter().all(|r| r.y == 20));
        assert!(rects[5..7].iter().all(|r| r.y == 40));
        // Rebalanced: rows 2 and 3 should each have two cells, not 3+1.
        assert_eq!(rects[3..5].len(), 2);
        assert_eq!(rects[5..7].len(), 2);
    }

    #[test]
    fn nine_cells_split_three_three_three() {
        let rects = layout(&slide_with_n(9), Rect::new(0, 0, 120, 60));
        assert_eq!(rects.len(), 9);
        assert_eq!(distinct_y_rows(&rects), 3);
        assert!(rects[0..3].iter().all(|r| r.y == 0));
        assert!(rects[3..6].iter().all(|r| r.y == 20));
        assert!(rects[6..9].iter().all(|r| r.y == 40));
    }

    #[test]
    fn narrow_two_cells_stack_vertically() {
        // 2 cells in 60-col area (would be 30+30): stack.
        let rects = layout(&slide_with_n(2), Rect::new(0, 0, 60, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(distinct_y_rows(&rects), 2);
        assert!(rects.iter().all(|r| r.x == 0 && r.width == 60));
    }

    #[test]
    fn wide_two_cells_remain_horizontal() {
        // 2 cells in 100-col area (50+50): horizontal (unchanged).
        let rects = layout(&slide_with_n(2), Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(distinct_y_rows(&rects), 1);
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn narrow_three_cells_stack_vertically() {
        // 3 cells in 90-col area (would be 30 each): stack.
        let rects = layout(&slide_with_n(3), Rect::new(0, 0, 90, 30));
        assert_eq!(rects.len(), 3);
        assert_eq!(distinct_y_rows(&rects), 3);
        assert!(rects.iter().all(|r| r.x == 0 && r.width == 90));
    }

    #[test]
    fn narrow_four_cells_stack_not_grid() {
        // 4 cells in 70-col area: vertical stack of 4, not 2×2 grid.
        let rects = layout(&slide_with_n(4), Rect::new(0, 0, 70, 40));
        assert_eq!(rects.len(), 4);
        assert_eq!(distinct_y_rows(&rects), 4);
        assert!(rects.iter().all(|r| r.x == 0 && r.width == 70));
    }

    #[test]
    fn three_cells_at_exact_threshold_remain_horizontal() {
        // 3 cells in 120-col area (40 each): three columns, no stack.
        let rects = layout(&slide_with_n(3), Rect::new(0, 0, 120, 20));
        assert_eq!(rects.len(), 3);
        assert_eq!(distinct_y_rows(&rects), 1);
        assert_eq!(rects[0], Rect::new(0, 0, 40, 20));
        assert_eq!(rects[1], Rect::new(40, 0, 40, 20));
        assert_eq!(rects[2], Rect::new(80, 0, 40, 20));
    }

    #[test]
    fn narrow_seven_cells_stack_vertically() {
        // n=7 in an area where max row (3 cols) would be narrow: stack all 7.
        let rects = layout(&slide_with_n(7), Rect::new(0, 0, 90, 70));
        assert_eq!(rects.len(), 7);
        assert_eq!(distinct_y_rows(&rects), 7);
        assert!(rects.iter().all(|r| r.x == 0 && r.width == 90));
    }

    #[test]
    fn very_narrow_single_column_area_still_stacks() {
        // Area narrower than MIN_COL_WIDTH even for one column: still stack.
        let rects = layout(&slide_with_n(2), Rect::new(0, 0, 20, 10));
        assert_eq!(rects.len(), 2);
        assert_eq!(distinct_y_rows(&rects), 2);
        assert!(rects.iter().all(|r| r.x == 0 && r.width == 20));
    }

    #[test]
    fn one_cell_unaffected_by_narrow_area() {
        // 1-cell case ignores MIN_COL_WIDTH entirely.
        let area = Rect::new(0, 0, 10, 10);
        let rects = layout(&slide_with_cells(vec![paragraph_cell()]), area);
        assert_eq!(rects, vec![area]);
    }

    #[test]
    fn row_distribution_matches_spec_examples() {
        assert_eq!(row_distribution(5), vec![3, 2]);
        assert_eq!(row_distribution(6), vec![3, 3]);
        assert_eq!(row_distribution(7), vec![3, 2, 2]);
        assert_eq!(row_distribution(8), vec![3, 3, 2]);
        assert_eq!(row_distribution(9), vec![3, 3, 3]);
        assert_eq!(row_distribution(10), vec![3, 3, 2, 2]);
        assert_eq!(row_distribution(11), vec![3, 3, 3, 2]);
    }
}
