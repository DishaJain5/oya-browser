/** Canonical callable identities and metadata are created once for every owned page realm. */
use super::*;
use crate::constants::*;
impl Default for Graph {
    /** Stable intrinsic roots exist even when source creates no closures. */
    fn default() -> Self {
        let kinds = [
            Intrinsic::FunctionConstructor,
            Intrinsic::FunctionPrototype,
            Intrinsic::Call,
            Intrinsic::Apply,
            Intrinsic::Bind,
        ];
        let mut functions = kinds
            .into_iter()
            .enumerate()
            .map(|(id, intrinsic)| {
                (
                    id as Id,
                    Function {
                        kind: Callable::Intrinsic { intrinsic },
                        properties: BTreeMap::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        functions
            .get_mut(&VM_FUNCTION_PROTOTYPE_ID)
            .expect("canonical prototype")
            .properties = BTreeMap::from([
            (
                "constructor".into(),
                JsValue::Function(VM_FUNCTION_CONSTRUCTOR_ID),
            ),
            ("call".into(), JsValue::Function(VM_FUNCTION_CALL_ID)),
            ("apply".into(), JsValue::Function(VM_FUNCTION_APPLY_ID)),
            ("bind".into(), JsValue::Function(VM_FUNCTION_BIND_ID)),
        ]);
        Self {
            environments: BTreeMap::new(),
            functions,
            next_environment: 0,
            next_function: VM_FUNCTION_INTRINSICS,
        }
    }
}
impl Intrinsic {
    /** Canonical identities pin selectors independently of mutable prototype data. */
    pub fn id(self) -> Id {
        match self {
            Self::FunctionConstructor => VM_FUNCTION_CONSTRUCTOR_ID,
            Self::FunctionPrototype => VM_FUNCTION_PROTOTYPE_ID,
            Self::Call => VM_FUNCTION_CALL_ID,
            Self::Apply => VM_FUNCTION_APPLY_ID,
            Self::Bind => VM_FUNCTION_BIND_ID,
        }
    }
    /** This table mirrors the standard builtin names and simple argument lengths. */
    pub(super) fn metadata(self) -> (&'static str, usize) {
        match self {
            Self::FunctionConstructor => ("Function", 1),
            Self::FunctionPrototype => ("", 0),
            Self::Call => ("call", 1),
            Self::Apply => ("apply", 2),
            Self::Bind => ("bind", 1),
        }
    }
}
