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
            Intrinsic::GetElementById,
            Intrinsic::AddEventListener,
            Intrinsic::RemoveEventListener,
            Intrinsic::PreventDefault,
            Intrinsic::StringTrim,
            Intrinsic::StringSplit,
            Intrinsic::ArrayMap,
            Intrinsic::ArrayFilter,
            Intrinsic::ArrayJoin,
            Intrinsic::ArraySome,
            Intrinsic::ArrayFind,
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
            Self::GetElementById => VM_GET_ELEMENT_BY_ID_ID,
            Self::AddEventListener => VM_ADD_EVENT_LISTENER_ID,
            Self::RemoveEventListener => VM_REMOVE_EVENT_LISTENER_ID,
            Self::PreventDefault => VM_PREVENT_DEFAULT_ID,
            Self::StringTrim => VM_STRING_TRIM_ID,
            Self::StringSplit => VM_STRING_SPLIT_ID,
            Self::ArrayMap => VM_ARRAY_MAP_ID,
            Self::ArrayFilter => VM_ARRAY_FILTER_ID,
            Self::ArrayJoin => VM_ARRAY_JOIN_ID,
            Self::ArraySome => VM_ARRAY_SOME_ID,
            Self::ArrayFind => VM_ARRAY_FIND_ID,
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
            Self::GetElementById => ("getElementById", 1),
            Self::AddEventListener => ("addEventListener", 2),
            Self::RemoveEventListener => ("removeEventListener", 2),
            Self::PreventDefault => ("preventDefault", 0),
            Self::StringTrim => ("trim", 0),
            Self::StringSplit => ("split", 2),
            Self::ArrayMap => ("map", 1),
            Self::ArrayFilter => ("filter", 1),
            Self::ArrayJoin => ("join", 1),
            Self::ArraySome => ("some", 1),
            Self::ArrayFind => ("find", 1),
        }
    }
}
