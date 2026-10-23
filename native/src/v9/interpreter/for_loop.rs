/** Standard loops preserve header scope, per-iteration let cells and completion-specific update order. */
use super::*;
use crate::v9::parser::ForLoop;
impl Interpreter {
    /** All loop exits, including initializer and update failures, restore the owning frame's scope depth. */
    pub(super) fn loop_for(
        &mut self,
        loop_: ForLoop,
        location: SourceLocation,
    ) -> EngineResult<Completion> {
        let base = self.locals.len();
        let id = self
            .enter_scope(ScopeId {
                site: location.byte_offset,
                kind: ScopeKind::For,
            })
            .map_err(|error| errors::located(error, location, "execute"))?;
        let fresh = self
            .graph
            .get(id)?
            .bindings
            .values()
            .any(|cell| cell.kind == BindingKind::Let);
        let result = self
            .execute_for(loop_, base, fresh, location)
            .map_err(|error| errors::located(error, location, "execute"));
        self.locals.truncate(base);
        result
    }
    /** Initialize once and copy the first iteration's let bindings before its condition. */
    fn execute_for(
        &mut self,
        loop_: ForLoop,
        scope: usize,
        fresh: bool,
        location: SourceLocation,
    ) -> EngineResult<Completion> {
        if let Some(initializer) = loop_.initializer {
            self.statement(*initializer)?;
        }
        self.fresh_iteration(scope, fresh)?;
        loop {
            self.step(location)?;
            if let Some(condition) = &loop_.condition
                && !self.expression(condition.clone())?.truthy()
            {
                return Ok(Completion::Normal);
            }
            match self.statement(*loop_.body.clone())? {
                Completion::Break => return Ok(Completion::Normal),
                returned @ Completion::Return(_) => return Ok(returned),
                Completion::Normal | Completion::Continue => {}
            }
            self.fresh_iteration(scope, fresh)?;
            if let Some(update) = &loop_.update {
                self.statement(*update.clone())?;
            }
        }
    }
    /** Replace, never accumulate, lexical cells while retaining referenced object identities. */
    fn fresh_iteration(&mut self, scope: usize, fresh: bool) -> EngineResult<()> {
        if fresh {
            let old = self.graph.get(self.locals[scope])?.clone();
            self.locals[scope] = self
                .graph
                .environment(old.scope_id, old.parent, old.bindings)?;
            self.environment()?;
        }
        Ok(())
    }
}
