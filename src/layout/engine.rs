use crate::parser::{Block, Cell, Slide};
use ratatui::layout::{Constraint, Layout, Rect};

/// Minimum width for any column in a multi-cell layout. When a proposed column
/// would be narrower than this, the whole slide falls back to a vertical stack.
pub(crate) const MIN_COL_WIDTH: u16 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellType {
    Image,
    Qr,
    Code,
    List,
    Prose,
    Heading,
    Empty,
}

impl CellType {
    fn rank(self) -> u8 {
        match self {
            CellType::Image | CellType::Qr => 5,
            CellType::Code => 4,
            CellType::List | CellType::Prose | CellType::Heading => 3,
            CellType::Empty => 0,
        }
    }
}

pub(crate) fn cell_type(cell: &Cell) -> CellType {
    let mut has_code = false;
    let mut has_list = false;
    let mut has_prose = false;
    let mut has_heading = false;
    for block in &cell.blocks {
        match block {
            Block::Image { .. } => return CellType::Image,
            Block::Qr { .. } => return CellType::Qr,
            Block::CodeBlock { .. } => has_code = true,
            Block::List { .. } => has_list = true,
            Block::Paragraph { .. } => has_prose = true,
            Block::Heading { .. } => has_heading = true,
        }
    }
    if has_code {
        CellType::Code
    } else if has_list {
        CellType::List
    } else if has_prose {
        CellType::Prose
    } else if has_heading {
        CellType::Heading
    } else {
        CellType::Empty
    }
}

fn two_cell_constraints(left: &Cell, right: &Cell) -> Vec<Constraint> {
    let r_left = cell_type(left).rank();
    let r_right = cell_type(right).rank();
    if r_left == r_right {
        vec![Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]
    } else if r_left > r_right {
        vec![Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)]
    } else {
        vec![Constraint::Ratio(2, 5), Constraint::Ratio(3, 5)]
    }
}

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
        let col_constraints: Vec<Constraint> = if n == 2 {
            two_cell_constraints(&slide.cells[0], &slide.cells[1])
        } else {
            (0..cols).map(|_| Constraint::Ratio(1, cols)).collect()
        };
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

    fn image_cell() -> Cell {
        Cell {
            blocks: vec![Block::Image {
                src: "x.png".into(),
                alt: String::new(),
                meta: None,
                span: span(),
            }],
            directives: vec![],
            span: span(),
        }
    }

    fn code_cell() -> Cell {
        Cell {
            blocks: vec![Block::CodeBlock {
                lang: None,
                source: "fn main() {}".into(),
                span: span(),
            }],
            directives: vec![],
            span: span(),
        }
    }

    fn list_cell() -> Cell {
        Cell {
            blocks: vec![Block::List {
                ordered: false,
                items: vec![],
                span: span(),
            }],
            directives: vec![],
            span: span(),
        }
    }

    fn heading_cell() -> Cell {
        Cell {
            blocks: vec![Block::Heading {
                level: 1,
                spans: vec![InlineSpan::Text("h".into())],
                span: span(),
            }],
            directives: vec![],
            span: span(),
        }
    }

    fn empty_cell() -> Cell {
        Cell {
            blocks: vec![],
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

    #[test]
    fn cell_type_classifies_each_block_kind() {
        assert_eq!(cell_type(&image_cell()), CellType::Image);
        assert_eq!(cell_type(&code_cell()), CellType::Code);
        assert_eq!(cell_type(&list_cell()), CellType::List);
        assert_eq!(cell_type(&paragraph_cell()), CellType::Prose);
        assert_eq!(cell_type(&heading_cell()), CellType::Heading);
        assert_eq!(cell_type(&empty_cell()), CellType::Empty);
    }

    #[test]
    fn cell_type_image_wins_over_other_blocks() {
        let cell = Cell {
            blocks: vec![
                Block::Paragraph {
                    spans: vec![InlineSpan::Text("p".into())],
                    span: span(),
                },
                Block::Image {
                    src: "x.png".into(),
                    alt: String::new(),
                    meta: None,
                    span: span(),
                },
            ],
            directives: vec![],
            span: span(),
        };
        assert_eq!(cell_type(&cell), CellType::Image);
    }

    #[test]
    fn cell_type_code_wins_over_list_prose_heading() {
        let cell = Cell {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    spans: vec![InlineSpan::Text("h".into())],
                    span: span(),
                },
                Block::Paragraph {
                    spans: vec![InlineSpan::Text("p".into())],
                    span: span(),
                },
                Block::CodeBlock {
                    lang: None,
                    source: "x".into(),
                    span: span(),
                },
            ],
            directives: vec![],
            span: span(),
        };
        assert_eq!(cell_type(&cell), CellType::Code);
    }

    #[test]
    fn prose_then_image_splits_40_60() {
        let slide = slide_with_cells(vec![paragraph_cell(), image_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 40, 20));
        assert_eq!(rects[1], Rect::new(40, 0, 60, 20));
    }

    #[test]
    fn image_then_prose_splits_60_40() {
        let slide = slide_with_cells(vec![image_cell(), paragraph_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 60, 20));
        assert_eq!(rects[1], Rect::new(60, 0, 40, 20));
    }

    #[test]
    fn code_then_list_splits_60_40() {
        let slide = slide_with_cells(vec![code_cell(), list_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 60, 20));
        assert_eq!(rects[1], Rect::new(60, 0, 40, 20));
    }

    #[test]
    fn list_then_code_splits_40_60() {
        let slide = slide_with_cells(vec![list_cell(), code_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 40, 20));
        assert_eq!(rects[1], Rect::new(40, 0, 60, 20));
    }

    #[test]
    fn image_image_splits_50_50() {
        let slide = slide_with_cells(vec![image_cell(), image_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(0, 0, 50, 20));
        assert_eq!(rects[1], Rect::new(50, 0, 50, 20));
    }

    #[test]
    fn code_code_splits_50_50() {
        let slide = slide_with_cells(vec![code_cell(), code_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn heading_then_prose_splits_50_50() {
        let slide = slide_with_cells(vec![heading_cell(), paragraph_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn list_then_heading_splits_50_50() {
        let slide = slide_with_cells(vec![list_cell(), heading_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn prose_then_list_splits_50_50() {
        let slide = slide_with_cells(vec![paragraph_cell(), list_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn empty_then_image_splits_40_60() {
        let slide = slide_with_cells(vec![empty_cell(), image_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects[0], Rect::new(0, 0, 40, 20));
        assert_eq!(rects[1], Rect::new(40, 0, 60, 20));
    }

    #[test]
    fn empty_then_prose_splits_40_60() {
        let slide = slide_with_cells(vec![empty_cell(), paragraph_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects[0], Rect::new(0, 0, 40, 20));
        assert_eq!(rects[1], Rect::new(40, 0, 60, 20));
    }

    #[test]
    fn empty_empty_splits_50_50() {
        let slide = slide_with_cells(vec![empty_cell(), empty_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn weighted_two_cell_preserves_reading_order() {
        // First cell is the higher-rank one; result[0] still corresponds to cells[0].
        let slide = slide_with_cells(vec![image_cell(), paragraph_cell()]);
        let rects = layout(&slide, Rect::new(0, 0, 100, 20));
        // cells[0] (image) sits on the left at x=0.
        assert_eq!(rects[0].x, 0);
        // cells[1] (prose) sits to the right.
        assert_eq!(rects[1].x, rects[0].width);
    }

    #[test]
    fn five_cells_trailing_two_row_stays_equal_split() {
        // n=5 splits 3+2; the trailing 2-cell row must NOT use the 2-cell weighting.
        let cells = vec![
            paragraph_cell(),
            paragraph_cell(),
            paragraph_cell(),
            image_cell(),
            paragraph_cell(),
        ];
        let rects = layout(&slide_with_cells(cells), Rect::new(0, 0, 120, 40));
        assert_eq!(rects.len(), 5);
        // Trailing row (indexes 3..5) splits 60/60, not 40/60.
        assert_eq!(rects[3].width, 60);
        assert_eq!(rects[4].width, 60);
    }
}
