use serde::Deserialize;

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
    Image {
        src: String,
        alt: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub blocks: Vec<Block>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageSize {
    Contain,
    Cover,
    FitWidth,
    FitHeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ImageMeta {
    #[serde(default)]
    pub size: Option<ImageSize>,
    #[serde(default)]
    pub x: Option<ImageAlign>,
    #[serde(default)]
    pub y: Option<ImageAlign>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
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
    Image {
        src: String,
        alt: String,
        meta: Option<ImageMeta>,
        span: SourceSpan,
    },
}

impl Block {
    pub fn span(&self) -> SourceSpan {
        match self {
            Block::Paragraph { span, .. } => *span,
            Block::Heading { span, .. } => *span,
            Block::List { span, .. } => *span,
            Block::Image { span, .. } => *span,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub blocks: Vec<Block>,
    pub directives: Vec<Directive>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    pub cells: Vec<Cell>,
    pub notes: Vec<Block>,
    pub directives: Vec<Directive>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideDeck {
    pub slides: Vec<Slide>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidImageMeta {
        span: SourceSpan,
        key: String,
        value: String,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidImageMeta { span, key, value } => write!(
                f,
                "invalid image metadata at {}..{}: {}={}",
                span.start, span.end, key, value
            ),
        }
    }
}

impl std::error::Error for ParseError {}
