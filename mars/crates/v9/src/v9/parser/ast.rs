/** Located scalar AST nodes share bounds and native DOM proposal semantics. */
use super::super::value::JsValue;
use super::super::{
    BindingKind,
    operator::{AssignmentOperator, Operator, UpdateOperator},
};
use crate::protocol::SourceLocation;
/** Each classic script owns its language mode independently of earlier scripts. */
pub(in crate::v9) struct Program {
    /// Source order remains significant for declaration visibility and effects.
    pub statements: Vec<Statement>,
    /// Classic scripts own independent directive prologues.
    pub strict: bool,
}
impl std::ops::Deref for Program {
    type Target = [Statement];
    /** Read-only scope walks reuse the ordered statement facade without copying code. */
    fn deref(&self) -> &Self::Target {
        &self.statements
    }
}
/** Every expression records its origin and actual tree depth before allocation. */
#[derive(Clone)]
pub(in crate::v9) struct Expression {
    pub kind: ExpressionKind,
    pub location: SourceLocation,
    pub depth: usize,
}
/** Expressions have no reflected host methods or arbitrary native calls. */
#[derive(Clone)]
pub(in crate::v9) enum ExpressionKind {
    Literal(JsValue),
    /** Receiver lookup is separate from author-visible lexical names. */
    This,
    /// Document mode resolves the request-owned host projection.
    Document,
    Variable(String),
    Input(String),
    Unary(Operator, Box<Expression>),
    Binary(Operator, Box<Expression>, Box<Expression>),
    Assignment(AssignmentOperator, Box<Expression>, Box<Expression>),
    Update(UpdateOperator, bool, Box<Expression>),
    Conditional(Box<Expression>, Box<Expression>, Box<Expression>),
    Array(Vec<ArrayElement>),
    Object(Vec<(String, Expression)>),
    Member(Box<Expression>, Box<Expression>),
    /** Callees are evaluated once, including returned and stored functions. */
    Invoke(Box<Expression>, Vec<Expression>),
    /** Boxing keeps scalar nodes small while source owns the function body. */
    Function(Box<FunctionLiteral>),
    StorageArea(StorageArea),
    StorageMethod(StorageArea, StorageMethod),
    StorageCall(StorageArea, StorageMethod, Vec<Expression>),
}
/** Source-owned spread markers retain their own diagnostics without executable wire data. */
#[derive(Clone)]
pub(in crate::v9) enum ArrayElement {
    /// Ordinary values keep their existing expression operation charge.
    Value(Expression),
    /// The ellipsis origin locates expansion failures independently of operand failures.
    Spread {
        /// Ordinary source expression accounting and registry identities remain intact.
        operand: Expression,
        /// Expansion errors retain the ellipsis rather than replacing child locations.
        location: SourceLocation,
    },
}
impl ArrayElement {
    /** Full source walks inspect every operand even when execution never reaches it. */
    pub fn operand(&self) -> &Expression {
        match self {
            Self::Value(operand) | Self::Spread { operand, .. } => operand,
        }
    }
    /** Evaluation consumes the source expression without cloning its function subtrees. */
    pub fn into_operand(self) -> Expression {
        match self {
            Self::Value(operand) | Self::Spread { operand, .. } => operand,
        }
    }
    /** Expansion errors belong to the ellipsis while ordinary errors retain the expression origin. */
    pub fn location(&self) -> SourceLocation {
        match self {
            Self::Value(operand) => operand.location,
            Self::Spread { location, .. } => *location,
        }
    }
}
/** Function code remains source-owned; only its source identity enters a closure record. */
#[derive(Clone)]
pub(in crate::v9) struct FunctionLiteral {
    /// Kind belongs to parsed source and never to a supplied function record.
    pub kind: FunctionKind,
    /// Source, including inherited directives, determines receiver conversion.
    pub strict: bool,
    /// Expression names get private self bindings rather than caller declarations.
    pub name: Option<String>,
    /// Observable names are source metadata, distinct from private recursive bindings.
    pub display_name: String,
    /// Simple parameters initialize before direct nested declarations.
    pub parameters: Vec<String>,
    /// The registry owns parsed code instead of accepting executable wire data.
    pub body: Vec<Statement>,
}
/** Arrow functions share lexical closures but do not declare ordinary implicit function bindings. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::v9) enum FunctionKind {
    Ordinary,
    Arrow,
}
/** Storage areas are coordinator-scoped capabilities, never script-selected keys. */
#[derive(Clone, Copy)]
pub(in crate::v9) enum StorageArea {
    Local,
    Session,
}
/** Only the four browser Storage string methods are callable. */
#[derive(Clone, Copy)]
pub(in crate::v9) enum StorageMethod {
    GetItem,
    SetItem,
    RemoveItem,
    Clear,
}
/** Statement locations remain available during event execution and error reporting. */
#[derive(Clone)]
pub(in crate::v9) struct Statement {
    pub kind: StatementKind,
    pub location: SourceLocation,
}
/** A missing var initializer preserves the instantiated cell rather than assigning undefined. */
#[derive(Clone)]
pub(in crate::v9) struct VarDeclarator {
    /// Simple identifiers select the containing declaration-scope binding.
    pub name: String,
    /// Initializers execute from left to right only when explicitly present.
    pub initializer: Option<Expression>,
    /// Each declarator retains an independent quota and execution error location.
    pub location: SourceLocation,
}
/** A standard loop owns optional clauses and one body without desugaring continue into the wrong target. */
#[derive(Clone)]
pub(in crate::v9) struct ForLoop {
    /** A lexical header binding is instantiated before this one-time clause runs. */
    pub initializer: Option<Box<Statement>>,
    /** Omission means true while the enclosing loop still charges work. */
    pub condition: Option<Expression>,
    /** This clause follows normal and continue completion, never break or return. */
    pub update: Option<Box<Statement>>,
    /** A single statement retains its own block scope and nearest-loop completion. */
    pub body: Box<Statement>,
}
/** Located statement variants retain the narrow native DOM boundary. */
#[derive(Clone)]
pub(in crate::v9) enum StatementKind {
    Var(Vec<VarDeclarator>),
    Binding(String, BindingKind, Expression),
    Set(String, Expression),
    Function(String, Box<FunctionLiteral>),
    Call(String, Vec<Expression>),
    Expression(Expression),
    Return(Option<Expression>),
    Block(Vec<Statement>),
    If(Expression, Box<Statement>, Option<Box<Statement>>),
    While(Expression, Box<Statement>),
    For(ForLoop),
    Break,
    Continue,
    Empty,
}
