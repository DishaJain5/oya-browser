/** One source-derived root schema keeps script instantiation and event validation in agreement. */
use super::*;
/** Recursive vars and direct functions share the global object's declaration environment. */
pub(super) fn object_names(program: &[Statement]) -> BTreeMap<String, SourceLocation> {
    let mut names = super::super::parser::var_declarations(program);
    for statement in program {
        if let StatementKind::Function(name, ..) = &statement.kind {
            names.insert(name.clone(), statement.location);
        }
    }
    names
}
/** Ordered scripts cannot redeclare lexical names through either declaration environment. */
pub(super) fn extend(
    globals: &Globals,
    objects: &BTreeMap<String, SourceLocation>,
    program: &[Statement],
) -> EngineResult<()> {
    for (name, location) in object_names(program) {
        if properties::protected_global(&name) {
            return Err(errors::unsupported(
                location,
                "global declarations replacing protected globals",
            ));
        }
        if globals.contains_key(&name) {
            return Err(conflict(location));
        }
    }
    for statement in program {
        if let StatementKind::Binding(name, ..) = &statement.kind
            && (globals.contains_key(name) || objects.contains_key(name))
        {
            return Err(conflict(statement.location));
        }
    }
    Ok(())
}
/** Root let/const cells retain TDZ state independently of global object properties. */
pub(super) fn lexical(program: &[Statement]) -> Globals {
    program
        .iter()
        .filter_map(|statement| {
            if let StatementKind::Binding(name, kind, _) = &statement.kind {
                Some((
                    name.clone(),
                    Binding {
                        kind: *kind,
                        value: None,
                    },
                ))
            } else {
                None
            }
        })
        .collect()
}
/** Every environment reports the same early-error category for a prohibited declaration overlap. */
fn conflict(location: SourceLocation) -> EngineError {
    errors::syntax(location, "Binding has already been declared in this scope")
}
