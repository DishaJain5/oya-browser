/** Pinned V8 grammar guides bounded lexical statements without executing partially parsed source. */
mod arrow;
mod assignment;
mod ast;
mod directives;
mod expression;
mod for_loop;
mod function;
mod scope;
mod statement;
mod strict;
mod var;
use super::{
    BindingKind, errors,
    lexer::{Kind, Token},
};
use crate::{
    constants::{STATEMENT_DEPTH, STATEMENTS},
    protocol::{EngineError, EngineResult, SourceLocation},
};
pub(super) use ast::{
    Expression, ExpressionKind, ForLoop, FunctionKind, FunctionLiteral, Program, Statement,
    StatementKind, StorageArea, StorageMethod, VarDeclarator,
};
pub(super) use var::var_declarations;

/** The entire program and every lexical scope are validated before candidate execution. */
pub(super) fn parse(tokens: Vec<Token>) -> EngineResult<Program> {
    parse_source(tokens, true)
}
/** Inline handlers cannot introduce code sites whose offsets collide with the page's source registry. */
pub(super) fn parse_handler(tokens: Vec<Token>) -> EngineResult<Vec<Statement>> {
    Ok(parse_source(tokens, false)?.statements)
}
/** Whole scripts and restricted named handlers share syntax and allocation bounds. */
fn parse_source(tokens: Vec<Token>, allow_functions: bool) -> EngineResult<Program> {
    let mut parser = Parser {
        parentheses: arrow::parentheses(&tokens),
        tokens,
        position: 0,
        statements: 0,
        nodes: 0,
        depth: 0,
        loops: 0,
        in_function: false,
        function_body: None,
        function_nesting: 0,
        allow_functions,
        strict: false,
    };
    let statements = parser.statement_list(Kind::End, true)?;
    scope::validate(&statements)?;
    strict::validate(&statements, parser.strict)?;
    Ok(Program {
        statements,
        strict: parser.strict,
    })
}
/** Source, syntax and loop nesting bounds are independent of eventual execution. */
struct Parser {
    tokens: Vec<Token>,
    /** One bounded prepass distinguishes potential parameter heads from ordinary grouping. */
    parentheses: Vec<Option<usize>>,
    position: usize,
    statements: usize,
    nodes: usize,
    depth: usize,
    loops: usize,
    in_function: bool,
    function_body: Option<usize>,
    function_nesting: usize,
    allow_functions: bool,
    /** Function parsing restores the defining mode on every completion. */
    strict: bool,
}
impl Parser {
    /** End remains available after an incomplete source construct. */
    fn peek(&self) -> &Token {
        &self.tokens[self.position.min(self.tokens.len() - 1)]
    }
    /** A single monotonic cursor consumes located tokens. */
    fn next(&mut self) -> Token {
        let token = self.peek().clone();
        if token.kind != Kind::End {
            self.position += 1;
        }
        token
    }
    /** Count and nesting are checked before recursing into any single-statement body. */
    fn statement(&mut self, list_item: bool) -> EngineResult<Statement> {
        self.statement_budget()?;
        if self.depth >= STATEMENT_DEPTH {
            return Err(self.limit("V9 statement depth", STATEMENT_DEPTH));
        }
        self.depth += 1;
        let result = self.statement_kind(list_item);
        self.depth -= 1;
        result
    }
    /** Parsed clauses and synthesized concise returns share one statement allocation cap. */
    fn statement_budget(&mut self) -> EngineResult<()> {
        self.statements += 1;
        if self.statements > STATEMENTS {
            return Err(self.limit("V9 statements", STATEMENTS));
        }
        Ok(())
    }
    /** Bindings are initialized only at their source position, after lexical instantiation. */
    fn binding(&mut self, kind: BindingKind) -> EngineResult<StatementKind> {
        let declaration = self.declaration(kind, false)?;
        self.symbol(';')?;
        Ok(declaration)
    }
    /** Header and ordinary declarations share identifier, initializer and TDZ grammar. */
    fn declaration(&mut self, kind: BindingKind, for_header: bool) -> EngineResult<StatementKind> {
        let name = self.identifier()?;
        if for_header {
            self.reject_for_each()?;
        }
        let mut value = if self.peek().kind == Kind::Symbol('=') {
            self.next();
            self.expression(0, 0)?
        } else if kind == BindingKind::Let {
            self.node(
                ExpressionKind::Literal(super::JsValue::Undefined),
                self.peek().location,
                1,
            )?
        } else {
            return Err(errors::syntax(
                self.peek().location,
                "const declarations require an initializer",
            ));
        };
        Self::infer_function_name(&mut value, &name);
        Ok(StatementKind::Binding(name, kind, value))
    }
    /** DOM writes remain proposals rather than general host object assignments. */
    fn assignment(&mut self) -> EngineResult<StatementKind> {
        let id = self.target()?;
        self.symbol('.')?;
        self.exact_name("textContent")?;
        if matches!(self.peek().kind, Kind::Assignment(_) | Kind::Update(_)) {
            return Err(errors::unsupported(
                self.peek().location,
                "compound or update stores to native DOM fields",
            ));
        }
        self.symbol('=')?;
        let value = self.expression(0, 0)?;
        self.symbol(';')?;
        Ok(StatementKind::Set(id, value))
    }
    /** Literal DOM lookup stays the only supported host receiver. */
    fn target(&mut self) -> EngineResult<String> {
        self.symbol('.')?;
        self.exact_name("getElementById")?;
        self.symbol('(')?;
        let token = self.next();
        let id = if let Kind::String(id) = token.kind {
            id.to_dom()
                .map_err(|e| errors::located(e, token.location, "dom"))?
        } else {
            return Err(errors::unsupported(token.location, "nonliteral DOM lookup"));
        };
        self.symbol(')')?;
        Ok(id)
    }
    /** Mismatches retain the original token location and never source excerpts. */
    fn symbol(&mut self, expected: char) -> EngineResult<()> {
        let token = self.next();
        if token.kind != Kind::Symbol(expected) {
            return Err(errors::syntax(
                token.location,
                "expected punctuation or an explicit semicolon",
            ));
        }
        Ok(())
    }
    /** Property names are terminals, not arbitrary JavaScript identifiers. */
    fn name(&mut self) -> EngineResult<String> {
        let token = self.next();
        if let Kind::Name(name) = token.kind {
            Ok(name)
        } else {
            Err(errors::syntax(token.location, "expected an identifier"))
        }
    }
    /** Reserved syntax cannot accidentally become a lexical cell or function. */
    fn identifier(&mut self) -> EngineResult<String> {
        let location = self.peek().location;
        let name = self.name()?;
        if reserved(&name) {
            return Err(errors::syntax(location, "reserved or conflicting binding"));
        }
        Ok(name)
    }
    /** Fixed DOM properties cannot reflect native methods. */
    fn exact_name(&mut self, expected: &str) -> EngineResult<()> {
        let location = self.peek().location;
        if self.name()? != expected {
            return Err(errors::unsupported(location, "this DOM property or method"));
        }
        Ok(())
    }
    /** All parse quotas report the token that would exceed them. */
    fn limit(&self, name: &str, bound: usize) -> EngineError {
        errors::located(
            EngineError::limit(name, bound),
            self.peek().location,
            "parse",
        )
    }
}
/** Keywords and the fixed host receiver cannot be reinterpreted as owned scalar bindings. */
fn reserved(name: &str) -> bool {
    [
        "let",
        "const",
        "var",
        "if",
        "else",
        "while",
        "break",
        "continue",
        "function",
        "return",
        "throw",
        "try",
        "catch",
        "finally",
        "for",
        "do",
        "switch",
        "case",
        "default",
        "class",
        "new",
        "this",
        "super",
        "import",
        "export",
        "extends",
        "in",
        "instanceof",
        "delete",
        "typeof",
        "void",
        "with",
        "debugger",
        "true",
        "false",
        "null",
        "enum",
        "document",
        "window",
    ]
    .contains(&name)
}
