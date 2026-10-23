/** Source-owned function code and lexical schemas validate private graph identities without executable wire data. */
use super::{
    BindingKind,
    parser::{
        Expression, ExpressionKind, FunctionKind, FunctionLiteral, Program, Statement,
        StatementKind, var_declarations,
    },
};
use crate::{
    constants::VM_FUNCTIONS,
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/** Scope roles distinguish a named expression's private name from its activation at the same source offset. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ScopeKind {
    Function,
    Block,
    For,
    Named,
}
/** Absolute source offsets are stable across parsing of the same ordered page scripts. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScopeId {
    /// Original byte offsets keep ordered scripts from sharing code identities.
    pub site: usize,
    /// One function site can own private-name and activation schemas.
    pub kind: ScopeKind,
}
/** Every retained environment must match the declaration shape of its exact source scope. */
pub(super) struct Scope {
    /// The defining source scope pins every runtime lexical-parent edge.
    pub parent: Option<ScopeId>,
    /// Serialized state cannot choose its cell names or mutability.
    pub bindings: BTreeMap<String, BindingKind>,
}
/** Function bodies stay process-owned; closure records refer only to this registry's keys. */
#[derive(Clone)]
pub(super) struct Code {
    /// Function kind is rebuilt from the code site; the private graph cannot change it.
    pub kind: FunctionKind,
    /// Effective strictness is rebuilt from directives, never accepted from private state.
    pub strict: bool,
    /// Positional names initialize before declarations are hoisted.
    pub parameters: Vec<String>,
    /// Executable statements come from source, never wire records.
    pub body: Vec<Statement>,
    /// Only named expressions create a private self-name context.
    pub name: Option<String>,
    /// Source inference never gives serialized closures authority to rename code.
    pub display_name: String,
    /// Captured environments must match this exact defining source scope.
    pub capture: Option<ScopeId>,
}
/** Code and scopes are rebuilt from validated source for every isolated service request. */
#[derive(Default)]
pub(super) struct Registry {
    /// Function records can name only these source-owned code offsets.
    pub codes: BTreeMap<usize, Code>,
    /// Retained environments match a schema and its lexical parent.
    pub scopes: BTreeMap<ScopeId, Scope>,
}
impl Registry {
    /** A whole page is inspected without running any initializer or storage operation. */
    pub fn build(programs: &[Program]) -> EngineResult<Self> {
        let mut registry = Self::default();
        for program in programs {
            registry.statements(program, None)?;
        }
        Ok(registry)
    }
    /** Function schemas own recursive vars; block and for schemas own only direct lexical declarations. */
    fn scope(
        &mut self,
        id: ScopeId,
        parent: Option<ScopeId>,
        body: &[Statement],
        parameters: &[String],
    ) {
        let mut bindings = parameters
            .iter()
            .map(|name| (name.clone(), BindingKind::Var))
            .collect::<BTreeMap<_, _>>();
        if id.kind == ScopeKind::Function {
            bindings.extend(
                var_declarations(body)
                    .into_keys()
                    .map(|name| (name, BindingKind::Var)),
            );
        }
        for statement in body {
            match &statement.kind {
                StatementKind::Binding(name, kind, _) => {
                    bindings.insert(name.clone(), *kind);
                }
                StatementKind::Function(name, ..) => {
                    bindings.insert(name.clone(), BindingKind::Var);
                }
                _ => {}
            }
        }
        self.scopes.insert(id, Scope { parent, bindings });
    }
    /** A function's capture schema is either its lexical parent or its own private name environment. */
    fn function(
        &mut self,
        site: usize,
        literal: &FunctionLiteral,
        parent: Option<ScopeId>,
    ) -> EngineResult<()> {
        if self.codes.contains_key(&site) {
            return Err(invalid());
        }
        if self.codes.len() >= VM_FUNCTIONS {
            return Err(EngineError::limit("V9 function code sites", VM_FUNCTIONS));
        }
        let capture = if let Some(name) = &literal.name {
            let id = ScopeId {
                site,
                kind: ScopeKind::Named,
            };
            self.scopes.insert(
                id,
                Scope {
                    parent,
                    bindings: BTreeMap::from([(name.clone(), BindingKind::Const)]),
                },
            );
            Some(id)
        } else {
            parent
        };
        self.codes.insert(
            site,
            Code {
                kind: literal.kind,
                strict: literal.strict,
                parameters: literal.parameters.clone(),
                body: literal.body.clone(),
                name: literal.name.clone(),
                display_name: literal.display_name.clone(),
                capture,
            },
        );
        let scope = ScopeId {
            site,
            kind: ScopeKind::Function,
        };
        self.scope(scope, capture, &literal.body, &literal.parameters);
        self.statements(&literal.body, Some(scope))
    }
    /** Iterative execution later uses the schema associated with each block or header location. */
    fn statements(&mut self, body: &[Statement], scope: Option<ScopeId>) -> EngineResult<()> {
        for statement in body {
            self.statement(statement, scope)?;
        }
        Ok(())
    }
    /** This walk visits every source branch, including functions never called. */
    fn statement(&mut self, statement: &Statement, scope: Option<ScopeId>) -> EngineResult<()> {
        let site = statement.location.byte_offset;
        match &statement.kind {
            StatementKind::Var(declarations) => {
                for declaration in declarations {
                    if let Some(value) = &declaration.initializer {
                        self.expression(value, scope)?;
                    }
                }
                Ok(())
            }
            StatementKind::Function(_, literal) => self.function(site, literal, scope),
            StatementKind::Block(body) => {
                let id = ScopeId {
                    site,
                    kind: ScopeKind::Block,
                };
                self.scope(id, scope, body, &[]);
                self.statements(body, Some(id))
            }
            StatementKind::For(loop_) => {
                let id = ScopeId {
                    site,
                    kind: ScopeKind::For,
                };
                let initializer = loop_
                    .initializer
                    .as_deref()
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>();
                self.scope(id, scope, &initializer, &[]);
                if let Some(init) = &loop_.initializer {
                    self.statement(init, Some(id))?;
                }
                if let Some(test) = &loop_.condition {
                    self.expression(test, Some(id))?;
                }
                if let Some(next) = &loop_.update {
                    self.statement(next, Some(id))?;
                }
                self.statement(&loop_.body, Some(id))
            }
            StatementKind::If(test, yes, no) => {
                self.expression(test, scope)?;
                self.statement(yes, scope)?;
                if let Some(no) = no {
                    self.statement(no, scope)?;
                }
                Ok(())
            }
            StatementKind::While(test, body) => {
                self.expression(test, scope)?;
                self.statement(body, scope)
            }
            StatementKind::Binding(_, _, value)
            | StatementKind::Set(_, value)
            | StatementKind::Expression(value) => self.expression(value, scope),
            StatementKind::Call(_, arguments) => self.expressions(arguments, scope),
            StatementKind::Return(Some(value)) => self.expression(value, scope),
            _ => Ok(()),
        }
    }
    /** Function literals nested inside arguments, keys or values receive their own source identities. */
    fn expressions(&mut self, values: &[Expression], scope: Option<ScopeId>) -> EngineResult<()> {
        for expression in values {
            self.expression(expression, scope)?;
        }
        Ok(())
    }
    /** No expression code is encoded into the private serialized state. */
    fn expression(&mut self, value: &Expression, scope: Option<ScopeId>) -> EngineResult<()> {
        match &value.kind {
            ExpressionKind::Function(literal) => {
                self.function(value.location.byte_offset, literal, scope)
            }
            ExpressionKind::Unary(_, value) | ExpressionKind::Update(_, _, value) => {
                self.expression(value, scope)
            }
            ExpressionKind::Binary(_, left, right)
            | ExpressionKind::Member(left, right)
            | ExpressionKind::Assignment(_, left, right) => {
                self.expression(left, scope)?;
                self.expression(right, scope)
            }
            ExpressionKind::Conditional(test, yes, no) => {
                self.expression(test, scope)?;
                self.expression(yes, scope)?;
                self.expression(no, scope)
            }
            ExpressionKind::Invoke(callee, args) => {
                self.expression(callee, scope)?;
                self.expressions(args, scope)
            }
            ExpressionKind::Array(elements) => {
                for element in elements {
                    self.expression(element.operand(), scope)?;
                }
                Ok(())
            }
            ExpressionKind::StorageCall(_, _, args) => self.expressions(args, scope),
            ExpressionKind::Object(properties) => {
                for (_, value) in properties {
                    self.expression(value, scope)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
/** Source-schema mismatches never disclose source or captured data. */
pub(super) fn invalid() -> EngineError {
    EngineError::invalid("Invalid V9 function or lexical environment state.")
}
