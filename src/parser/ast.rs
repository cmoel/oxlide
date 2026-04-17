#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSpan {
    Text(String),
    Strong(Vec<InlineSpan>),
    Emphasis(Vec<InlineSpan>),
    Code(String),
    Link {
        url: String,
        text: Vec<InlineSpan>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub blocks: Vec<Block>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph {
        spans: Vec<InlineSpan>,
        span: SourceSpan,
    },
    Heading {
        level: u8,
        spans: Vec<InlineSpan>,
        span: SourceSpan,
    },
    List {
        ordered: bool,
        items: Vec<ListItem>,
        span: SourceSpan,
    },
}

impl Block {
    pub fn span(&self) -> SourceSpan {
        match self {
            Block::Paragraph { span, .. } => *span,
            Block::Heading { span, .. } => *span,
            Block::List { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    Raw {
        name: String,
        args: String,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub blocks: Vec<Block>,
    pub directives: Vec<Directive>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slide {
    pub cells: Vec<Cell>,
    pub notes: Vec<Block>,
    pub directives: Vec<Directive>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideDeck {
    pub slides: Vec<Slide>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for ParseError {}
