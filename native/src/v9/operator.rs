/** Typed operators keep longest-token scanning and precedence in one owned language table. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operator {
    TypeOf,
    Void,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Not,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
impl Operator {
    /** Longer spellings precede their prefixes; unsupported operators are checked first. */
    pub fn scan(source: &str) -> Option<(&'static str, Self)> {
        [
            ("===", Self::Equal),
            ("!==", Self::NotEqual),
            ("<=", Self::LessEqual),
            (">=", Self::GreaterEqual),
            ("&&", Self::And),
            ("||", Self::Or),
            ("+", Self::Add),
            ("-", Self::Subtract),
            ("*", Self::Multiply),
            ("/", Self::Divide),
            ("%", Self::Remainder),
            ("!", Self::Not),
            ("<", Self::Less),
            (">", Self::Greater),
        ]
        .into_iter()
        .find(|(spelling, _)| source.starts_with(spelling))
    }
    /** ECMAScript binding strengths are compressed without changing relative precedence. */
    pub fn precedence(self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Equal | Self::NotEqual => 3,
            Self::Less | Self::LessEqual | Self::Greater | Self::GreaterEqual => 4,
            Self::Add | Self::Subtract => 5,
            Self::Multiply | Self::Divide | Self::Remainder => 6,
            Self::Not | Self::TypeOf | Self::Void => 0,
        }
    }
}
/** Assignment tokens select retained-reference stores without duplicating target evaluation. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AssignmentOperator {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}
/** Count expressions preserve the numeric old value independently from their store. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UpdateOperator {
    Increment,
    Decrement,
}
