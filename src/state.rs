/// Which quote character delimited the attribute value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
    Double,
    Single,
}

/// Sub-state within DOCTYPE content scanning.
///
/// Tracks whether the scanner is inside a comment, PI, or quoted string
/// so that `[`, `]`, and `>` inside those constructs are not misinterpreted
/// as structural delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctypeSubState {
    /// Normal scanning - `[`, `]`, `>` are structural.
    Normal,
    /// Saw `<` - checking for `!` (comment) or `?` (PI).
    AfterLt,
    /// Saw `<!` - checking for `-`.
    AfterLtBang,
    /// Saw `<!-` - expecting `-` to enter comment.
    AfterLtBangDash,
    /// Inside `<!-- ... -->`, tracking consecutive dashes for exit.
    Comment { dash_count: u8 },
    /// Inside `<? ... ?>`, tracking whether last byte was `?`.
    PI { saw_qmark: bool },
    /// Inside a double-quoted string.
    DoubleQuoted,
    /// Inside a single-quoted string.
    SingleQuoted,
}

/// Parser state: tracks which XML construct we are currently inside.
///
/// The `usize` fields store buffer-relative positions (not absolute stream offsets).
/// They are converted to absolute offsets when emitting events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserState {
    /// Between markup. Scanning for `<` or `&` in text content.
    Content,

    /// Saw `<`, determining what kind of markup follows.
    AfterLt,

    /// Inside a start tag, reading the tag name.
    /// `name_start` is the buffer position of the first name character.
    StartTagName { name_start: usize },

    /// After the tag name in a start tag, before attributes or `>`.
    StartTagPostName,

    /// Reading an attribute name.
    /// `name_start` is the buffer position of the first name character.
    AttrName { name_start: usize },

    /// After attribute name, expecting `=`.
    AfterAttrName,

    /// After `=`, expecting a quote character.
    BeforeAttrValue,

    /// Inside a quoted attribute value.
    /// Position tracking uses `content_start` on Reader.
    /// `quote` records which delimiter character opened the value.
    AttrValue { quote: QuoteStyle },

    /// Entity reference inside an attribute value: after `&`, scanning for `;`.
    /// `name_start` is the buffer position of the first character after `&`.
    /// `quote` records which AttrValue state to return to after `;`.
    AttrEntityRef { name_start: usize, quote: QuoteStyle },

    /// Character reference inside an attribute value: after `&#`, scanning for `;`.
    /// `value_start` is the buffer position of the first character after `&#`.
    /// `quote` records which AttrValue state to return to after `;`.
    AttrCharRef { value_start: usize, quote: QuoteStyle },

    /// Inside `</`, reading the end tag name.
    /// `name_start` is the buffer position of the first name character.
    EndTagName { name_start: usize },

    /// After end tag name, expecting `>`.
    EndTagPostName,

    /// Saw `/` in a start tag, expecting `>` to complete `/>`.
    /// Used when `/` appears at the end of a buffer and `>` is in the next chunk.
    StartTagGotSlash,

    // --- Phase 2 states (comments, PIs, CDATA, DOCTYPE, references) ---

    /// After `<!`, need to determine comment, CDATA, or DOCTYPE.
    AfterLtBang,

    /// After `<!-`, expecting second `-` to start a comment.
    AfterLtBangDash,

    /// Comment content: scanning for `-->`.
    /// `dash_count` tracks consecutive dashes at the scanning position.
    CommentContent { dash_count: u8 },

    /// After `<![`, matching `CDATA[`.
    /// `matched` is how many characters of "CDATA[" have been matched so far.
    AfterLtBangBracket { matched: u8 },

    /// CDATA content: scanning for `]]>`.
    /// `bracket_count` tracks consecutive `]` at the scanning position.
    CdataContent { bracket_count: u8 },

    /// After `<!D`, matching `OCTYPE`.
    /// `matched` is how many characters of "OCTYPE" have been matched.
    AfterLtBangD { matched: u8 },

    /// After `<!DOCTYPE `, reading the root element name.
    /// `name_start` is the buffer position of the first name character.
    DoctypeName { name_start: usize },

    /// DOCTYPE content: scanning for `>` with balanced `[`/`]`.
    /// `depth` tracks bracket nesting depth.
    /// `sub` tracks comment/PI/quote context to avoid misinterpreting delimiters.
    DoctypeContent { depth: u32, sub: DoctypeSubState },

    /// Processing instruction: reading target name after `<?`.
    /// `name_start` is the buffer position of the first name character.
    PITarget { name_start: usize },

    /// Processing instruction content: scanning for `?>`.
    /// `saw_qmark` is true if the last character seen was `?`.
    PIContent { saw_qmark: bool },

    /// Entity reference in text content: after `&`, scanning for `;`.
    /// `name_start` is the buffer position of the first character after `&`.
    EntityRef { name_start: usize },

    /// Character reference: after `&#`, scanning for `;`.
    /// `value_start` is the buffer position of the first character after `&#` or `&#x`.
    CharRef { value_start: usize },
}

impl ParserState {
    /// Adjust all buffer-relative positions after the caller shifts the buffer
    /// by `consumed` bytes. Only states with `usize` position fields need updating.
    pub fn adjust_positions(&mut self, consumed: usize) {
        match self {
            ParserState::StartTagName { name_start } => *name_start -= consumed,
            ParserState::AttrName { name_start } => *name_start -= consumed,
            ParserState::AttrEntityRef { name_start, .. } => *name_start -= consumed,
            ParserState::AttrCharRef { value_start, .. } => *value_start -= consumed,
            ParserState::EndTagName { name_start } => *name_start -= consumed,
            ParserState::DoctypeName { name_start } if *name_start < usize::MAX - 1 => *name_start -= consumed,
            ParserState::PITarget { name_start } => *name_start -= consumed,
            ParserState::EntityRef { name_start } => *name_start -= consumed,
            ParserState::CharRef { value_start } => *value_start -= consumed,
            // States without buffer-relative positions:
            ParserState::Content
            | ParserState::AfterLt
            | ParserState::StartTagPostName
            | ParserState::StartTagGotSlash
            | ParserState::AfterAttrName
            | ParserState::BeforeAttrValue
            | ParserState::AttrValue { .. }
            | ParserState::EndTagPostName
            | ParserState::AfterLtBang
            | ParserState::AfterLtBangDash
            | ParserState::CommentContent { .. }
            | ParserState::AfterLtBangBracket { .. }
            | ParserState::CdataContent { .. }
            | ParserState::AfterLtBangD { .. }
            | ParserState::DoctypeName { .. }
            | ParserState::DoctypeContent { .. }
            | ParserState::PIContent { .. } => {}
        }
    }
}
