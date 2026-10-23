# Oya Native

Oya Native is Oya's Rust browser, being built to replace the Electron browser for people and
agents. Its network, document/layout, JavaScript runtime, native window and agent tools are
implemented in Rust, with the engines running as supervised services. It uses no Chromium,
Blink, V8, CDP, WebView, injected agent scripts or existing browser engine.

The replacement target is a complete everyday browser with native human controls and native
agent tools over the same page. This build is in development and does not yet match Electron's
browser compatibility. The implementation-status sections below describe what works today and
what remains to build; they do not narrow the product goal.

## Performance and footprint targets

The long-term target is for V9 to use no more than one tenth of V8's memory, run comparable
JavaScript work in no more than one tenth of its time, and occupy no more than one tenth of its
runtime size. For the complete native browser, the corresponding target is one tenth of the
current Electron browser's memory, elapsed time for equivalent work, and installed size. These
are unmeasured goals, not claims about this build. Browser comparisons count only when both
products complete the same workload with equivalent results; current compatibility is still in
development. The architecture document defines the comparison method and workload boundaries.

## Run

From the repository root, with Rust 1.94 or newer:

```sh
cargo run --offline --manifest-path native/Cargo.toml -- desktop --url demo
cargo run --offline --manifest-path native/Cargo.toml -- desktop --url forms
cargo run --offline --manifest-path native/Cargo.toml -- desktop --agent --url demo
cargo run --offline --manifest-path native/Cargo.toml -- demo
cargo run --offline --manifest-path native/Cargo.toml -- agent
```

The window requires macOS. Click **Open demo**, enter a name and click **Say hello**. V9 reads
the input and changes the greeting. Follow the second-page link, then try Back, Forward,
Reload, scrolling and resizing. The address field accepts supported public HTTP/HTTPS pages.
Unsupported syntax produces an error and retains the last successfully committed page.

Offline builds need the locked crates cached. On a fresh machine, omit `--offline` once to
download ordinary Rust dependencies; no browser engine is downloaded. AppKit supplies native
window/widgets/font shaping and drawing. Rustls supplies verified TLS, webpki-roots its trust
roots, url URL syntax, serde serialization, and libc cancellable OS pipe polling. Oya owns
HTTP framing, parsing, document state, layout, language execution and native agent authority.
The macOS desktop stores supported session cookies in the system Keychain; clear them from the
Oya Native menu with **Clear Saved Cookies…**. This profile does not import Electron cookies.
On an ordinary launch, the desktop restores a bounded ordered list of HTTP(S) tabs after secure
profile recovery succeeds. Restored tabs use GET and start under human control. An explicit `--url`
launch takes precedence and leaves the saved session unchanged for that run. Session URLs are
stored in a separate Keychain item; query and fragment data are hidden from agent page views.
If the final tab-session write fails on close, the desktop offers retry, keep-open, or explicit
close without saving.

## Native agent connection

`desktop --agent` connects stdin/stdout to the **visible window**, using versioned NDJSON.
Diagnostics use stderr. One request receives one correlated response containing exactly one
of `result` or `error`. Frames and parameter schemas are bounded and strict. The headless
`agent` command retains its local-only single-owner contract.

```json
{"version":2,"id":"discover","op":"runtime.capabilities","params":{}}
{"version":2,"id":"control","op":"control.status","params":{}}
```

The native toolbar's **Allow agent** grants control; **Take control** immediately revokes queued
and in-flight agent mutations. Agents can inspect without control and release control, but
cannot grant it to themselves. `--agent-control` is an explicit operator startup grant for
unattended sessions. Every desktop agent mutation also requires `control_epoch` from
`control.status`. A null-id service_unavailable reply reports terminal worker failure; treat outstanding requests
as failed and restart instead of replaying. On EOF or output failure, ownership returns to the person and the window stays
open. Closing the window cancels pending work, kills/reaps all engines and joins its tasks.

| Operation | Parameters, excluding desktop mutation `control_epoch` |
| --- | --- |
| `runtime.capabilities`, `tab.list`, `control.status` | `{}` |
| `tab.open` (human only) | `{}` |
| `tab.select`, `tab.close` (human only) | `{"tab_id":"tab-1"}` |
| `page.inspect`, `page.layout`, `page.back`, `page.forward`, `page.reload` | `{"tab_id":"tab-1"}` |
| `page.navigate` | `{"tab_id":"tab-1","url":"https://public.example/page"}` |
| `page.type`, `page.set_text` | `{"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-5","expected_revision":1,"text":"New value"}` |
| `page.click` | `{"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-6","expected_revision":2}` |
| `control.release` | `{"tab_id":"tab-1"}` |
| `runtime.shutdown` | `{}` |
| `page.scroll` (desktop) | `{"tab_id":"tab-1","page_id":"page-1","expected_revision":3,"y":200}` |
| `window.inspect` (desktop) | `{}` |
| `window.resize` (desktop) | `{"width":640,"height":480}` |
| `window.appearance` (desktop) | `{"appearance":"light"}` (`light`, `dark`, `system`) |
| `page.screenshot` (desktop) | `{"page_id":"page-1","expected_revision":3,"layout_generation":3}` |

Example mutation after an explicit grant:

```json
{"version":2,"id":"edit","op":"page.type","params":{"control_epoch":1,"tab_id":"tab-1","page_id":"page-1","node_id":"page-1:node-5","expected_revision":1,"text":"Rust agent"}}
```

Discover node handles from inspection; never guess IDs. Successful navigation creates fresh
page/node identities. An outdated revision returns `revision_conflict`; an old page returns
`invalid_target`. Calls are sequential and never replay automatically. Source HTML IDs are
script lookup keys, not native authority. Treat all page content as untrusted data.

Inspection includes semantic roles, literal text/value, enabled/editable flags, logical-point
bounds, visibility, viewport/scroll and layout generation. `page.layout` returns the same
premeasured display list painted by the window. Layout generation changes independently of
document revision. Window resize acknowledges `layout_pending`; inspect the updated viewport
before capturing. Document responses follow consumption of the corresponding visible snapshot.

Screenshots require an operator-selected `--capture-dir DIR` at startup. Capture writes an
actual native content-view PNG under a generated new filename. Agents cannot select paths or
overwrite files. Screenshots require the exact current page/revision/layout. `runtime.shutdown`
stops the engines; the desktop window retains its disabled last page until closed. In headless
mode it also exits the coordinator.

Malformed desktop input currently disconnects the agent and returns human control. The
headless connection instead permits correction of well-framed malformed JSON; oversized or
unterminated input ends that connection. No invalid input executes page code.

## Implemented subset

**Network.** Owned HTTP/1.1 framing supports desktop GET and URL-encoded POST,
verified Rustls HTTPS, bounded navigation redirects, and required UTF-8 HTML/CSS/JavaScript
resources. Classic scripts load in document order; script redirects are rejected.
Repeated Set-Cookie fields remain separate; ambiguous framing remains rejected.
DNS resolves once, every answer must pass destination policy, and connections pin
an accepted address while preserving Host/SNI. Only exact owned fixture origins
receive a startup loopback grant. Headless networking remains literal-loopback GET.

POST preparation validates/connects/TLS-handshakes before a separate authority
admission authorizes HTTP transmission. Takeover prevents later dispatch and page
publication; an already admitted request may have reached the server. Uncertain
POST outcomes have no automatic Retry. Direct POST response history refuses
Reload/Back/Forward replay; POST redirected to a final GET can revisit that GET.

The desktop stores its bounded host-only cookie jar as one versioned macOS Keychain
item. Session-only cookies persist across app restarts; expiry and server deletion
still apply. **Clear Saved Cookies…** in the Oya Native menu removes saved and live
cookies after confirmation. If secure storage cannot be read, cookie-bearing
navigation is blocked until retry or clear succeeds. If secure storage cannot persist
the recovery lock after a failed clear, the browser warns that cookies may remain and
must stay open until a clear succeeds. Cookie values never enter agent inspection or
diagnostics. Headless runs remain in memory only. Cookies check
paths, Secure/prefix requirements, SameSite, expiry/deletion and insecure replacement.
Domain cookies are refused; same-site matching conservatively requires the same
scheme and exact hostname. This does not provide Electron profile import or general
production session compatibility. Cookie commits require current authority,
independently of subsequent document parsing. Password-bearing GET chains omit
query/fragment from surfaced URLs.

**Document/forms.** Owned bounded tokenization supports comments, case-insensitive
names, single/double/unquoted attributes, common character references and omitted
wrappers/selected optional closings. Inert attributes remain data. This is not a
complete HTML5 tree builder. Static tables support captions, groups, rows, cells,
and basic header associations; spans, nested tables and advanced table CSS remain
unsupported. Other unsupported embeds, modules and
asynchronous/deferred scripts fail explicitly. Up to eight ordered inline or external classic
scripts are supported within the existing V9 language and source budgets.
Static PNG and JPEG resources load through the governed network service, with
bounded encoded/decoded sizes, intrinsic geometry, native AppKit painting,
agent image roles/bounds and alt-text fallback on ordinary resource failure.
SVG, animation, CSS backgrounds, responsive selection and lazy loading remain
unsupported.

Forms support labels, text/password/hidden values, checkbox/radio state, one-option
select, textarea, disabled/read-only controls and submitters. Submission preserves
field order/repeated names, excludes unsuccessful controls, normalizes newlines,
and performs GET or URL-encoded POST. Only implemented required constraints are
validated. Password/hidden values are omitted from inspection; protected values
are unavailable to the current V9 DOM-read seam.

**CSS/layout.** Required inline/linked styles use an owned basic cascade for
simple type/class/ID/descendant selectors, source order/specificity/importance,
text styles, colors, spacing, widths, borders and visibility. Owned block/inline
flow uses native font metrics; the same display list supplies paint and agent
bounds. Unsafe unsupported selectors/layout/conditions refuse publication rather
than exposing hidden controls. Some decoration produces bounded diagnostics.
This remains a partial CSS implementation with compatibility limitations.

**V9.** The [source map](UPSTREAM.md) pins the V8 parser and bytecode-generator
routines used to guide this Rust slice. The bounded language includes typed
numbers, strings, booleans, null and undefined; object and array literals;
own-property reads and writes; `let`/`const`/`var`; first-class functions, anonymous/named
function expressions, directly nested declarations, callbacks and lexical closures
with shared captured bindings; parameters, returns and recursive calls; `if`,
`while`, `break` and `continue`;
arithmetic, comparisons and short-circuit logic; explicit input reads and
`textContent` proposals. Scripts require explicit semicolons.

`let` cells are mutable; `const` cells require an initializer and refuse reassignment.
Bindings exist from entry to their lexical block and fail before initialization,
including in their own initializer. Each invocation sees its own blocks and the
defining lexical environments and page globals rather than its caller's local variables.
Sibling closures share captured cells, while separate factory calls are independent.
Named function expressions keep their recursive self-name private. Assigning that
self-name remains explicitly unsupported. Functions can be assigned, passed,
returned, stored in objects/arrays and invoked through variables, parentheses,
returned functions and own-property member expressions. Member calls preserve the object receiver. Ordinary functions use the call-site receiver;
arrows retain their defining `this`, including after return and serialized native events. Function identity and truthiness
are supported; function-to-primitive conversion is not. Inline handlers retain
the named-call front door, including positional arguments; they cannot introduce
function literals or general expression callees. Boolean, null and undefined
values retain their types. Supported comparison/logical operators are `===`, `!==`,
`<`, `<=`, `>`, `>=`, `!`, `&&` and `||`; the latter two short-circuit and return an
operand value. Blocks, `if`/`else`, `while`, `break` and `continue` share the bounded
execution budget. Even empty infinite loops stop explicitly. Standard
`for (initializer; condition; update)` loops admit omitted clauses, one lexical
`let`/`const` declaration, a `var` declaration list or a supported expression/assignment in the initializer,
and supported expression/assignment updates. `let` headers get fresh shallow
per-iteration cells while object identities remain shared. `continue` runs the
update; `break` and return skip it. Lexical header names disappear when the loop finishes;
`var` headers keep the containing function/script cell, shared by every iteration.
`for…in`, `for…of`, `for await`, multiple lexical declarators, general comma expressions,
coercive equality, logical/nullish/bitwise/exponentiation assignments and optional
chaining remain unsupported.

Simple `var` declarations belong to the containing function or classic script,
including declarations inside blocks, loops and unreachable branches. All their
cells exist with an undefined value before execution. Optional initializers run
left to right; a declaration without one preserves the current value. Repeated
vars share their cell with parameters and supported direct function declarations.
Functions initialize before statements; the last direct duplicate supplies the
function value until an executed initializer or assignment replaces it. Lexical
name conflicts remain early errors, and nested functions own their own vars.
Each classic script instantiates when reached; later scripts are not hoisted into
earlier execution. Every declarator consumes the existing declaration budget,
including repeated names. The page permits at most 64 distinct global object
bindings separately from the existing 64 lexical-global limit.

Simple assignment expressions; arithmetic compound assignments (`+=`, `-=`, `*=`,
`/=`, `%=`); prefix and postfix numeric updates (`++`, `--`); conditional expressions;
`typeof` and `void` are supported. Assignment targets remain supported identifiers
and own-property member references. Plain assignment returns its assigned value
and associates right to left. Compounds retain the original receiver/key and old
value before running the right-hand expression. Updates convert supported
primitives to Number; prefix returns the new Number and postfix the converted old
Number, including negative zero. Object/function numeric conversion stays
explicitly unsupported. Existing const, TDZ, strict, protected-global, array and
readonly function-metadata store checks still apply.

Conditional expressions evaluate one arm. `typeof` reports supported value types
and returns `"undefined"` for an unresolved ordinary identifier, while TDZ,
implicit-arguments and unavailable host values still fail explicitly. `void`
evaluates its operand once and returns undefined. Postfix updates cannot cross a
line break, including one inside a comment. Inline handlers retain the named-call
boundary; native DOM writes keep their dedicated `textContent = ...` statement
form, without compound assignments or updates on host fields. All expression
children, including call arguments and unselected branches, participate in source
depth validation; only executed operands consume the runtime work budget.

Arrow functions support `name => expression`, `() => expression` and simple
parenthesized parameter lists, including a trailing comma. Concise expression bodies
return their value implicitly; block bodies use explicit `return` and otherwise
return undefined. Nested arrows capture defining lexical cells and ordinary functions
can coexist in the same script. No line break is allowed before `=>`; line breaks
after it are allowed. Arrow recognition uses assignment precedence, so a binary
operand needs grouping, for example `false || (x => x)`.
Function kind comes from parsed source, never a supplied graph field. Arrows create
no own `this` or implicit `arguments` binding; lexical `this` is captured from the defining context. Explicitly declared lexical/parameter
`arguments` values work; implicit arguments objects remain unsupported. An ordinary
function's `var arguments` is explicitly unsupported unless an explicit parameter
or direct function declaration supplies that binding; arrows may own such a var.
An ordinary activation that would supply an implicit arguments object blocks
fallback to outer cells; a same-activation explicit binding is resolved normally.

Exact unescaped `"use strict";` or `'use strict';` in a script/function directive
prologue activates strict mode. Escaped, grouped or later strings do not. Functions
inherit their defining source mode, including arrow block directives; each classic
script starts its own mode. Strict ordinary detached calls receive undefined;
non-strict detached calls receive the page-global object. Top-level `this` remains
that object even in strict classic scripts. Restricted strict identifiers are early
errors, including in unreachable bodies. Explicit semicolons remain required.

One page-owned ordinary heap object represents the supported script-global receiver.
`globalThis` resolves to it unless lexically shadowed. Root var/function names read/write
the same cells through identifiers and receiver properties; lexical `let`/`const`
cells are absent from the object. Other page-authored own properties resolve through
bare identifiers, and unresolved non-strict assignments create properties. Strict
unresolved assignments fail. The global object is always a collector root. Arrow
captures and invocation receivers are source-shape-validated references and share
existing string/state/allocation budgets.

This object is not a complete Window/WindowProxy. `window` retains only the fixed
Storage facade. Global native-host properties and writes to protected names
(`globalThis`, `undefined`, `NaN`, `Infinity`, `window`, `document`, `fetch`, `eval`,
`require`, `process`, `localStorage`, `sessionStorage`) fail explicitly as unsupported,
as do root var/function declarations replacing them. Lexical shadowing remains separate.
No native shell, agent handle or network authority is reachable through receivers.
Primitive receiver boxing, arbitrary prototype replacement and broader reflection remain unsupported.

The demo's **Add one** button retains a numeric counter across events and switches
to **Target reached.** at three. A standard `for` loop computes the displayed sum
from one to the current count; **Reset counter** restores zero. These use the
page's own Rust-executed script and ordinary native document updates.
The script uses `count++`, `i++`, `sum += i` and a conditional branch value.

The **Methods and saved callbacks** greeting form calls an ordinary method on an
object controller. The returned arrow reads and writes the controller through `this`,
while accepting an input-processing callback. Its count survives separate **Say hello**
events after the factory and method return. The count is independent of **Reset counter**;
reload creates a fresh controller. Empty names default to “world”. The controller
saves a bound ordinary method with a greeting prefix. Each event uses `apply` for
current inputs and `call` for name processing; a local arrow captures the controller receiver.
The saved callback uses a script-global `var`; an earlier read observes its hoisted
undefined value. Local var redeclarations retain the processed name without changing
the visible greeting journey.

Function values expose first-class `call`, `apply` and `bind` through a shared,
callable Function prototype. Extracted methods retain their identity and require a
callable method receiver. Own data properties can shadow inherited methods; changing
the supported prototype method properties affects subsequent lookups. `Function`
resolves to the canonical constructor unless shadowed or replaced; dynamic function
construction remains explicitly unsupported. Calling `Function.prototype` returns
undefined. No native handles are reachable through this function layer.

`call` forwards an explicit receiver and positional arguments. `apply` accepts null
or undefined for an empty list, supported arrays (including holes), and ordinary
object/function array-likes. It reads length once, applies primitive ToLength
conversion and checks the argument cap before allocation or indexed reads. Object
conversion for length remains unsupported. Missing indexed values become undefined.

`bind` creates a distinct callable retaining its target, receiver and prefixed
arguments across events. Rebinding keeps the original inner receiver and puts inner
prefixes before outer prefixes. Arrow targets retain lexical this. Bound chains are
limited to 16 hops, with at most 16 cumulative arguments. Every invocation, including
intrinsic adaptation, shares operation and call-depth budgets.

Supported function `name` and `length` are readonly own metadata. Non-strict writes
are ignored; strict writes fail. Names come from declarations, explicit expression
names, direct anonymous declarators/identifier assignments and fixed object-literal
properties. Other anonymous expressions keep empty names. Binding prefixes the name
with “bound ” and subtracts prefixed arguments from length, down to zero. Descriptor
redefinition, deletion and general reflection APIs remain unsupported; this does
not claim those APIs or complete prototype behavior.

Each page snapshot carries lexical cells and flat, bounded object, function and
environment arenas, so aliases and cycles survive service serialization. Parsed
source owns executable code and exact scope schemas; private graphs carry only
IDs and data. Environment chains are checked iteratively before execution.
Event execution mutates a candidate snapshot; neither captured-cell/heap changes nor DOM proposals commit if any later
operation fails. Heap, property, array length, source, token, call-depth and
wire-byte limits are reported through the existing located error surface.
There are at most 512 allocated function records, 512 environments and 2,048
environment cells per candidate, with 16 levels of syntactic function nesting.
The 128 KiB private-state ceiling also applies. Successful request boundaries trace
and reclaim unreachable records across all three arenas, including cycles, while
preserving monotonic IDs. Long-running requests can exhaust an allocation cap
before the operation budget; collection does not run during evaluation.
V9 supports `window.localStorage` and `window.sessionStorage` with only
`getItem`, `setItem`, `removeItem` and `clear`. Both areas hold string keys and
values, up to 256 entries and 64 KiB per origin-area (2,048 UTF-16 units per
key/value). Local storage is origin-scoped and kept in a separate Keychain item;
session storage is scoped to the tab and origin and disappears when the tab closes.
Cookie clearing leaves local storage intact. Storage writes remain candidates
until script execution, document validation, layout and authority publication succeed.
The aggregate localStorage record is limited to 512 KiB. Secure-store failures block
storage use rather than falling back to plaintext. An uncertain write or rollback also stores a
separate recovery marker. Startup checks it before loading site values; explicit recovery clears
the origin map before removing the marker. Cookie clearing does not touch either Web Storage item.
Object coercion, general object prototypes, primitive
wrappers, async arrows, constructors,
implicit arguments objects, rest/default/destructured parameters,
block-function declarations, generators, general DOM
access, exceptions, modules, asynchronous execution, BigInt, templates, regex,
legacy octal and non-ASCII identifiers remain unsupported. This remains a
current bounded JavaScript subset, not V8 or Test262 conformance. The pinned `ryu-js`
primitive supplies Number formatting only. Lone surrogates remain in VM state;
the current UTF-8 document boundary refuses them explicitly.
No complete V8, Test262, Blink or current-Oya compatibility is claimed. All script
changes remain candidate data until document validation and authority publication.

[Upstream notices](THIRD_PARTY_NOTICES.md) accompany adapted material.

## Bounds and lifecycle

| Resource | Bound |
| --- | --- |
| JSON frame including escaping/newline | 256 KiB |
| HTML / HTTP headers | 64 KiB / 8 KiB |
| Nodes / nesting / UTF-8 bytes per node value | 512 / 32 / 2 KiB |
| V9 per-script / page aggregate source / script count / tokens / statements / operations / call depth / parameters | 4 KiB / 32 KiB / 8 / 1,024 / 64 / 2,048 / 16 / 16 |
| V9 objects / own properties / array length / state bytes | 256 / 1,024 / 1,024 / 128 KiB |
| Expression recursion/AST depth / statement nesting / JS string / environment strings | 32 / 32 / 2,048 UTF-16 units / 16,384 UTF-16 units |
| Stylesheets / each sheet / aggregate CSS | 4 linked / 16 KiB / 64 KiB |
| Session cookies / per host / aggregate jar / cookie Keychain record | 64 / 16 / 32 KiB / 48 KiB |
| Web Storage entries / area / per key or value / aggregate Keychain record | 256 / 64 KiB / 2,048 UTF-16 units / 512 KiB |
| URL / correlation ID | 2 KiB / 128 bytes |
| Display lines / history entries / desktop command queue | 2,048 / 64 / 16 |
| Headless network / ordinary engine RPC | 2 s / 3 s |
| Desktop network budget / outer network RPC deadline | 8 s / 10 s |

DNS and TLS internal I/O remain inside the supervised network process; the outer deadline
kills the runtime if they cannot return in time. Cancellation polls at most every 25 ms while
waiting for child replies. A service/transport failure retires all three engines without replay.
Ordinary navigation errors retain the previous page and report previous_page_retained:true.
History records only successful navigations; back/forward move only after successful load.

Services use inherited private pipes, not public network listeners. They have cleared
environments and no agent credentials, persistence or grandchildren. Process separation is
**not an OS sandbox**. The current build has no imported Electron profiles,
downloads, media, frames, extensions, cloud deployment or full JavaScript. Killing only the coordinator is detected independently by each engine
through an expected-parent watchdog, including while OS DNS or another dispatch is blocked.

## Verify

```sh
cargo fmt --manifest-path native/Cargo.toml -- --check
cargo clippy --offline --all-targets --all-features --manifest-path native/Cargo.toml -- -D warnings
cargo test --offline --all-features --manifest-path native/Cargo.toml
cargo test --offline --all-features --manifest-path native/Cargo.toml --test window -- --ignored --nocapture --test-threads=1
```

The last command deliberately opens the actual macOS window and saves native screenshots.
GUI checks require `test-support` and use fresh in-memory cookie, storage and session profiles;
they do not read or write the user's Keychain profile. Their startup-only `--test-profile`
and `--verify-ui` flags are absent from ordinary builds. The verification deadline covers tabs
as well as field/button actions. Normal desktop launches keep the system profile.
Other tests are hermetic; loopback fixtures use synthetic in-memory data and no external sites
or .env. TLS test key/certificate are disposable, deliberately untrusted fixtures. The optional
test-support executable exists only for child-fault tests.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership, boundaries and implementation adaptations.
