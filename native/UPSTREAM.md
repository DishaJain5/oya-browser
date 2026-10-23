# Source-guided Rust engines

The existing prototype is not a Rust port of V8, Blink or Chromium networking.
The replacement target remains the behavior of the current Oya browser. An
implemented subset is not evidence of full browser compatibility.

## Pinned references

The project's engine documentation pins Chromium
`2c592105bbcd9490a9894df48d0fe59b2c512651`. Its
[DEPS](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/DEPS)
pins V8 `fedd021b538607860f9c56e5c193ad8dcd4f2bb1`.
These references identify the code being studied, rather than following a
moving branch. They are not runtime dependencies.

| Source inspected | Observed responsibilities | Rust destination |
| --- | --- | --- |
| [V8 scanner.cc](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/scanner.cc): constructor, token lookahead, comment skipping, ScanHexNumber, ScanEscape, ScanOctalEscape and ScanString | UTF-16 source/literals; exact escape decoding; comment and line-terminator distinctions; explicit illegal tokens | `src/v9/lexer/`, with a bounded source cursor and typed literals |
| [V8 parser-base.h](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/parsing/parser-base.h): `ParsePrimaryExpression`, `ParseAssignmentExpression`, `ParseArrayLiteral`, `ParseObjectLiteral`, `ParseProperty`, `ParseArguments`, `ParseFormalParameterList` | Primary/assignment grammar, literal property order, call arguments and formal parameters | `src/v9/parser/expression.rs` and `parser/statement.rs`; no bytecode or full grammar port |
| [V8 bytecode-generator.cc](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/interpreter/bytecode-generator.cc): `VisitIfStatement`, `VisitFunctionLiteral`, `VisitObjectLiteral`, `VisitArrayLiteral`, `BuildAssignment`, `VisitProperty`, `VisitCall` | Branch, function, object/array, assignment, property and call evaluation paths | `src/v9/interpreter/` and `value/heap.rs`; Rust evaluates the supported subset directly and does not generate bytecode |
| [V8 arithmetic builtin dispatch](https://github.com/v8/v8/blob/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-number-gen.cc) | Arithmetic delegates to binary/unary operation assemblers; this dispatch alone does not establish conversion semantics | `src/v9/value/`; conversion contract is taken from the normative operations below |
| [Blink html_document_parser.cc](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/third_party/blink/renderer/core/html/parser/html_document_parser.cc): context tokenizer modes and initial scheduling collaborators | Separate data/RCDATA/raw-text/script modes; tokenizer, tree builder, script runner, resource loader and parser scheduling cooperate | `src/document/` and the document service; present parser is still a prototype |
| [Chromium network_service.h](https://chromium.googlesource.com/chromium/src/+/2c592105bbcd9490a9894df48d0fe59b2c512651/services/network/network_service.h): service/context interfaces | Network service/context lifetimes; resolver, TLS, authentication and decoding interfaces; explicit test seams | `src/network/` and per-context coordinator ownership; present networking is still partial |

The parser and interpreter references above are from the pinned V8 revision.
They guide the grammar and evaluation order for the supported slice; they do not
establish full ECMAScript semantics or a line-by-line source port.

The lexical/control-flow layer additionally inspects these routines at that same
revision, with the downloaded source checked locally where the browser's source
view was truncated:

| Reviewed routines | Rust adaptation |
| --- | --- |
| `parser-base.h`: `ParseVariableDeclarations` (4521–4704), `ParseBlock` (5988–6034), `ParseIfStatement` (6183–6220), `ParseContinueStatement`/`ParseBreakStatement` (6223–6298), `ParseWhileStatement` (6409–6437) | let/const initialization grammar, block ownership, nearest-if else binding and enclosing-loop validation |
| `parser-base.h`: `ParseLogicalExpression`, `ParseBinaryContinuation`, `ParseBinaryExpression` (3536–3554, 3703–3779) | Typed operator precedence and left association for the supported operators |
| `bytecode-generator.cc`: `VisitBlockDeclarationsAndStatements`/`VisitVariableDeclaration` (2119–2179), `BuildThrowIfHole`/`BuildHoleCheckForVariableAssignment`/`BuildVariableAssignment` (4904–5100), `VisitAssignment` (5955 onward) | Instantiate lexical cells before statements; distinct initialization and assignment; resolve a store before its RHS, then enforce TDZ/const rules |
| `bytecode-generator.cc`: `VisitIfStatement` (2502–2541), Continue/Break visitors (2549–2559), `VisitWhileStatement` (3189–3208), logical OR/AND visitors (8460–8555) | Selected-branch evaluation, propagated loop completion and operand-preserving short circuit |

Rust executes the AST directly; this does not port Ignition's bytecode generation,
register allocation or optimizations. Function activations follow their defining
lexical environments rather than caller-local environments.
Event-state validation checks the exact source-declared global names/kinds without
replaying initializers. The coordinator still owns the trust and publication boundary.
The primitive and lexical cases in `tests/unit/v9/control.rs` are newly authored,
not an imported Test262 or V8 suite. Normative ToBoolean/ToNumber/ToString rules
were read alongside the pinned implementation references.

Standard `for` additionally follows pinned `parser-base.h` `ParseForStatement`
(6668–6828), `ParseStandardForLoopWithLexicalDeclarations` (6939–6997) and
`ParseStandardForLoop` (6997–7041), plus `bytecode-generator.cc`
`VisitForStatement` (3210–3258). The Rust AST retains explicit initializer, test,
update and body clauses. Execution owns a bounded header scope, copies `let`
cells per iteration, routes continue through update and skips update on break,
return or error. This adapts observed scope and execution ordering; it does not
port V8's closure desugaring or bytecode implementation. Closures now retain the
distinct iteration records. `tests/unit/v9/for_loop.rs` contains
newly authored cases, not an imported upstream conformance suite.

The first-class-function layer additionally inspects these pinned sources:

| Reviewed routines | Rust adaptation |
| --- | --- |
| `parser-base.h`: `ParseFunctionExpression` (4183–4220) | Anonymous/named expressions, simple parameter/body parsing and a private expression name; generators and full grammar remain unimplemented; arrows are described below |
| `bytecode-generator.cc`: `VisitFunctionLiteral` (3496–3524), `BuildVariableLoad` (4708–4806), `BuildNewLocalActivationContext`/context initialization/block context creation (8648–8742), `VisitCall` callee/receiver setup (6855–6998) | Source-owned code sites paired with captured contexts, lexical lookup with TDZ cells, activation/block records and callee-before-arguments evaluation; no bytecode or optimization machinery; receiver handling is described below |
| [V8 contexts.h](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/contexts.h) (453–493, 546–602, 590–726, 954–1013) | Closures pair code with a context; contexts link to lexical parents and distinguish uninitialized bindings. Rust keeps ordinary typed cells and does not port optimization-specific ContextCell states |
| [V8 js-function.tq](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/js-function.tq): `JSFunction.shared_function_info` and `context` | Function values identify parsed code and the defining environment; no native pointers or executable AST crosses the private wire |

`src/v9/registry.rs` rebuilds code/scope schemas from the same ordered source.
`graph/` validates flat function/environment references, lexical parents, exact
cell schemas and named self bindings. Activations and per-iteration environments
retain shared cells for closures after their active IDs are popped. At successful
request finish, an owned iterative tracer sweeps unreachable objects, functions
and environments together, including cycles. This simple collector is newly
authored Rust code, not a port of V8's garbage collector. No collection runs while
evaluator temporaries remain live. Per-request allocations have explicit caps.
`tests/unit/v9/closures.rs`, `graph.rs` and `tests/closures.rs` are newly authored
behavior and malformed-state cases, not imported upstream conformance suites.

The arrow-function layer additionally reads pinned `parser-base.h`
`NewFunctionScope` (890–907), `UseThis` (1305–1320),
`ParseAssignmentExpressionCoverGrammar`/continuation (3290–3383),
`ParsePrimaryExpression` arrow-head handling (2170–2310),
`ValidateFormalParameters` (1295–1301), `ParseFunctionBody` (4917–4967)
and `ParseArrowFunctionLiteral` (5108–5278). These routines were inspected in
the complete local copy acquired at the pinned revision. Oya browser navigation
to GitHub and googlesource timed out during this increment; no substitute browser
was used or successful web read claimed.

Rust recognizes arrows only at assignment-expression entry. A bounded token
prepass pairs parentheses so potential heads can be distinguished from ordinary
grouping without speculative reparsing. Simple names and parameter lists are
validated before bodies; token line coordinates enforce no line terminator before
`=>`, including in comments. Concise bodies synthesize a located return charged
to the existing statement and expression budgets. Shared function-context cleanup
isolates loop targets and declaration nesting on every completion.

`FunctionKind` is source-owned AST/registry metadata. Wire function records still
contain only code/environment IDs, and duplicate registry sites are rejected.
Arrows reuse the same closure cells, activation, invocation, validation and tracing
paths. V8's skipped default function bindings and enclosing receiver lookup inform
lexical receiver capture described below; no implicit arguments object is fabricated.
Explicit lexical/parameter `arguments` cells resolve normally; a lookup requiring
an ordinary activation's implicit arguments object is unsupported. Arguments
lookup stops at that ordinary activation before considering outer `arguments`
cells, while explicitly declared cells within it retain TDZ and assignment rules.
Arguments objects, async arrows and richer parameters remain unimplemented.
The new `tests/unit/v9/arrows.rs` and `arrows/grammar.rs` cases are authored here,
not imported upstream or full arrow conformance tests.

The receiver layer additionally inspects pinned `parser-base.h` `UseThis`
(1305–1320), `ParseStatementList` (5761–5829), and `bytecode-generator.cc`
`VisitCall` (6855–6998). Oya successfully opened and read the pinned source view
for this increment. The large arm64 file was truncated by that text view, so the
same pinned file was acquired as source through GitHub's contents API. The complete
`src/builtins/arm64/builtins-arm64.cc` `Generate_CallFunction` (2946–3030) was then
read locally: strict calls preserve receivers; sloppy null/undefined uses the
callee realm global proxy, and other primitives require ToObject.

Rust retains member base/key/callee references ahead of arguments, captures lexical
arrow receivers and converts detached ordinary calls according to source-owned
strictness. Exact raw string-token provenance and direct prologue statements set
program/function mode; inherited strictness and restricted-name early errors are
checked before execution. Caller mode never chooses callee mode. The AST evaluator
uses a page-owned heap global object with coherent existing root-function cells;
it does not port V8's global proxy, native context, machine-code call stub or
ToObject wrapper implementation. Window/WindowProxy, primitive wrappers, protected
global mutation and general reflection remain explicit unsupported limits.

`graph/` validates and traces arrow captures and invocation receivers, including
receiver-only object reachability. The canonical global object is always rooted.
The existing candidate boundary publishes receiver/global properties, captured
cells, heap, Storage and DOM together. `tests/unit/v9/receivers.rs` and the added
real-service rollback case are newly authored behavior tests, not imported V8 or
Test262 conformance suites.

The callable-object layer additionally reads pinned
[`function.tq::FastFunctionPrototypeBind`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/function.tq),
[`builtins-function.cc::DoFunctionBind`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/builtins-function.cc),
`builtins-arm64.cc::Generate_FunctionPrototypeCall`, `Generate_FunctionPrototypeApply`
and `Generate_CallBoundFunctionImpl`, plus
[`js-function.cc::CopyNameAndLength`, `JSBoundFunction::GetName` and `GetLength`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/objects/js-function.cc).
The complete relevant routines were read through Oya; the larger js-function file
was truncated after those routines, so local excerpt line numbers are not upstream
line references. [`conversion.tq::ToLength`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/builtins/conversion.tq)
was read for NaN/nonpositive handling, positive flooring and maximum-safe-integer
clamping. The already pinned parser's `SetFunctionNameFromIdentifierRef` and
`SetFunctionNameFromPropertyName` call sites guide supported name inference.

Rust retains stable intrinsic identities and a shared callable Function prototype,
while source closures and bound callbacks use the owned graph. Binding stores the
target/receiver/prefix values and derives readonly name/length metadata in the
reviewed order. Calls preserve callee-before-arguments evaluation; apply constructs
a bounded array-like list; bound-chain invocation preserves inner binding order.
Canonical identities, function properties and bound edges are validated, counted
and traced alongside existing closures. This does not port V8 machine-code stubs,
maps, accessors, optimization or general object/prototype machinery. Dynamic
Function construction, bound constructors, primitive wrappers and descriptor
reflection remain explicit limits. The new metadata, call and graph cases and
native bound-callback integrations are authored here, not imported conformance suites.

Var declaration instantiation additionally follows the pinned parser's
`ParseVariableDeclarations` (declaration-scope ownership), the already reviewed
`BytecodeGenerator::VisitVariableDeclaration` (lexical initialization versus var),
and [`runtime-scopes.cc::DeclareGlobal` and `Runtime_DeclareGlobals`](https://chromium.googlesource.com/v8/v8/+/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/src/runtime/runtime-scopes.cc).
The complete global-declaration routines were read through Oya before the larger
file was truncated; local excerpt lines are not upstream line numbers. Existing
global vars retain values, new vars begin undefined, direct functions initialize
before execution, and lexical collisions refuse instantiation. Rust reuses a
source walker and its owned cell graph rather than V8 declaration arrays, contexts,
global dictionaries or property descriptors. Protected host globals, full global
descriptor semantics, destructuring, implicit arguments objects and Annex-B block
functions remain outside this slice. The var unit and real-service cases are newly
authored behavior tests, not imported conformance suites.

The ordinary-expression slice uses additional routines in those same pinned
parser and bytecode-generator files: `parser-base.h` assignment continuation
(3292–3445), conditional continuation (3675–3700), unary/prefix (3783–3843) and
unary/postfix (3876–3934); `bytecode-generator.cc` conditional (4110–4144),
assignment/compound (5955–6088), void/typeof (7349–7372) and count operation
(7514–7705). These routines guide right-associative assignment, lazy conditional
branches, retained references and old values for compound stores, numeric postfix
results, and special unresolved-name handling for `typeof`.

Rust uses one synchronous prepared-reference path over existing cells and arena
IDs. It does not reproduce V8 bytecode, machine-code operations or optimization.
Primitive conversion and protected-host restrictions remain the existing bounded
ones; general object coercion, accessors, logical/bitwise/exponentiation assignments,
automatic semicolon insertion and arbitrary DOM property stores remain outside
this slice. New expression tests are authored here rather than imported V8 or
Test262 cases.

## Implementation evidence

For each adapted algorithm, retain required upstream notices, name the exact
source routine and revision, record deliberate differences, and test through
the public Rust service. Test provenance must distinguish adapted upstream
cases from newly authored conformance cases. Passing a selected case set is
not a V8, Test262, Blink or web-platform conformance claim.

The Rust implementation carries typed scalar values, flat heap object IDs,
lexical bindings and DOM proposals through page load and native events. It is
source-guided, not a line-by-line translation of V8. The normative
[ToNumber/StringToNumber/ToString operations](https://tc39.es/ecma262/multipage/abstract-operations.html#sec-tonumber)
were reviewed for conversion requirements. The selected decimal cases in
`tests/unit/v9/expressions.rs` adapt the inspected decimal section of pinned
[number-tostring.js](https://github.com/v8/v8/blame/fedd021b538607860f9c56e5c193ad8dcd4f2bb1/test/mjsunit/number-tostring.js);
the remaining expression/lexical cases there are newly authored.

`ryu-js` is pinned to 1.0.3. Its package and Buffer implementation were inspected
before integration; it supplies Number-to-string formatting only. Other numeric
syntax/conversions, UTF-16 storage, scanning, parsing and interpretation are
owned Rust code. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) retains notices.

Deliberate limits: ASCII identifiers; bounded own data properties and sparse
arrays; no general object prototypes, getters/setters or object coercion; function declarations
only at program root or directly inside function bodies, plus anonymous/named
expressions with lexical closures;
no async arrows, rest/default/destructured parameters, primitive receiver wrappers, implicit
arguments objects, constructors, block functions, BigInt, templates,
regular expressions or Annex-B legacy octal; explicit semicolons. Unpaired
surrogates survive in VM state, but the current UTF-8 document boundary refuses
them explicitly. Full V8 semantics, broader DOM/CSS, media, profiles, Oya
workflow integration and production platform protections remain open.
