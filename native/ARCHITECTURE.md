# Independent native Rust browser

The Rust browser is being built as a complete replacement for Oya's Electron browser for people
and agents. Current implementation areas are under development and not yet at compatibility parity.
It has no route into the existing Electron product, server, persona store or published CLI.

```text
AppKit native window       native agent NDJSON
        \                    /
       bounded commands + local control epochs
                        |
          one worker / one Rust Runtime
          bounded tab states + shared engines and cookie profile
                        |
            supervised private pipes
               /        |        \
          network    document     V9
          process     process   process
             |           |
      OS sockets/TLS    owned layout + OS glyph metrics
```

## Ownership and transactions

The main thread owns NSWindow, widgets, immutable BrowserSnapshot and display. It cannot create
another document runtime. One worker owns Runtime and all three engines. A bounded queue combines
human and agent requests. Only the composition root chooses executables, fixture grants, initial
ownership and the optional capture directory.

Fetch → parse → V9 load/event → document validate/apply → native display → wire budget validation
→ authority recheck and atomic publication. Document and V9 operations are stateless over bounded
values. Networking owns one expiring prepared connection. Transmission has its own authority
admission after connection/TLS preparation; response cookies commit under current authority
independently of subsequent document parsing. POST responses carry an explicit no-replay history
policy. In the macOS desktop, one bounded host-only cookie jar is serialized as a versioned
Keychain item. A response candidate is stored before it becomes live; a stale authority permit
causes storage rollback, and failed rollback blocks further navigation. Session-only cookies
survive app restarts until deletion, expiry where present, or an explicit clear. Headless sessions
remain in memory only.
No candidate appears in the committed page before every step succeeds. Navigation advances page
generation/history; mutations advance revision; resizing changes layout generation independently.
Failed navigation keeps the page/history position. History refetches rather than storing DOMs.
Web Storage is supplied to V9 as a bounded candidate. Local values are keyed by canonical origin
in a distinct Keychain record; session values are keyed by tab and origin. A storage write is
persisted before publication, then rolled back if the exact tab permit is stale. An uncertain
write or failed rollback sets a separate Keychain recovery marker; startup checks it before reading
site values. Recovery deletes the origin map before the marker, so late writes cannot become trusted
on restart. Cookie clearing touches only the cookie item.

Desktop tabs each own page identity, V9 state, history cursor and saved scroll. They share one
network/document/V9 process trio, viewport and secure cookie profile. Agent page requests name an
exact, never-reused `tab_id`; background work temporarily loads that tab's state and restores the
human selection afterward. An ordinary launch restores a bounded ordered HTTP(S) URL list from a
third Keychain item after cookie and Web Storage recovery succeeds. Restored pages use GET, receive
fresh tab IDs and begin under human control; query and fragment data stay hidden from agent views.
An explicit startup URL skips session restoration and saving for that run. Malformed session data
falls back to a fresh tab. Concurrent app processes use last-complete-write-wins session snapshots.
Tab ownership epochs are independent, so takeover and close revoke
the relevant pending work; app close invalidates every queued tab mutation before the final save.
Activating a saved page rerenders its layout at the current
viewport and clamps its scroll. The native strip bounds the session to the configured tab limit;
multiple windows, tab history restoration and closed-tab recovery remain future work.

## Performance and footprint targets

The long-term target is at least a 10× improvement in memory use, elapsed time for comparable
work, and runtime size for V9 relative to the pinned V8 revision. The whole native browser targets
at most one tenth the process-tree memory, elapsed time for equivalent work, and installed size
of the current Electron browser. These targets have not been demonstrated. Browser-level results
count only after both products complete the same workload with equivalent outcomes.

Comparisons must use equivalent release builds on the same hardware and OS, with the same
optimization and instrumentation settings. Count the complete process trees, including the
native coordinator and all three services. Predefine startup-to-first-render, fixed-resource
navigation-to-render, and V9 parse-and-execute workloads. Report cold and warm latency percentiles,
peak and steady-state memory under the same OS metric, and installed and compressed-download
bytes separately. V9 comparisons use only compatible scripts with matching observable results;
browser comparisons exclude user data and caches and use the same page/resource set.

Authority is an owner/epoch mutex with per-tab owner epochs. Admissions capture a Permit. Human
takeover, tab close and agent EOF immediately invalidate relevant work, including during network
I/O. Publication checks the exact tab identity and owner epoch under the same brief lock as commit,
without engine I/O. Agents also supply control_epoch and exact page/node/revision. Human edits
preserve native order and can rebase revision only within their originally observed page generation.
Deleted identities are not recycled within a page.

Native controls use the engine boxes; custom painting uses its premeasured lines. Inspection
comes from that same display. AppKit does not parse HTML, execute JS or choose page flow. Native
captures read the actual root view after display completion. A capture repository writes
create-new PNGs beneath a startup-only directory grant.

## Modules

| Module | Responsibility |
| --- | --- |
| protocol/ | Strict envelopes, errors and bounded NDJSON |
| network/ | Loopback client; desktop policy, exact-IP sockets, Rustls and owned HTTP framing |
| document/ | Bounded tree, atomic text/value changes, image metadata and native-point layout |
| v9/ | Tokenizer, located AST, source code/scope registry, captured lexical cells, bounded value arenas and interpretation |
| runtime/supervisor.rs | Readiness, pipe workers, deadlines, cancellation and reaping |
| runtime/navigation.rs | Candidates, final redirected URL and atomic history |
| runtime/interaction.rs | Revision-scoped type/click/set-text and V9 events |
| runtime/page.rs, images.rs | Sole committed page, bounded image assets, snapshots and output budgets |
| runtime/presentation.rs | Shared layout, viewport and scroll |
| runtime/authority.rs | Owner epochs and admission/publication validation |
| profile.rs | Versioned cookie record and macOS Keychain repository; no plaintext fallback |
| desktop/broker.rs, desktop/io.rs | Bounded queues and cancellable agent pipes |
| desktop/window/ | Native intentions, ordered updates, OS-only tools, menus |
| desktop/canvas.rs, desktop/chrome.rs | Native controls, painting and shell |
| platform/ | OS font measurement/drawing primitives |

## Failure, egress and lifetime

Each engine has one bounded transport worker. The supervisor retains Child handles outside
blocking I/O, so deadline/cancellation kills the process to release both pipes, then joins the
worker. Transport/protocol/type failure or child exit retires the runtime. Ordinary unsupported
input/navigation errors leave it healthy. No automatic restart or replay occurs.

Desktop requests resolve once, reject any nonpublic DNS answer (including mixed sets), then
connect to an exact accepted address. Host/SNI remain the URL hostname. Every redirect repeats
policy, with no TLS downgrade. Only the owned demo's exact origin gets a startup loopback grant.
Headless policy remains literal-loopback HTTP. Proxy environment is not inherited.

Keychain reads/writes/deletes run on one guarded helper thread with a three-second result
deadline. A timeout retires profile access rather than blocking the browser worker. Failed clear
or rollback writes a bounded recovery marker so restart remains blocked. If Keychain rejects both
the operation and marker, the current runtime stays blocked and explicitly tells the person to
keep the browser open until a clear succeeds; restart safety cannot be guaranteed while every
secure-store write is failing.

The window owns engine and demo-site lifetime. Close signals cancellation, kills/reaps engines
and joins the worker, transport and pollable agent I/O tasks. Agent EOF revokes ownership while
the window remains open. Blocked stdout cannot block the main thread; reply saturation revokes
control. Every service also has an expected-parent watchdog: reparenting exits the process even
while its main dispatch is blocked in OS DNS. Event-stream disconnection revokes ownership,
clears Loading, disables stale controls and shows explicit restart instructions. Private pipes establish local ownership, not an OS sandbox or remote authentication.

## Dependencies and adaptations

Locked serde crates supply serialization; url supplies syntax; Rustls/ring and webpki roots
supply verified TLS; png and jpeg-decoder validate static image resources; security-framework
binds the macOS Keychain; objc2 supplies AppKit bindings; libc supplies cancellable pipe polling.
No HTTP client, JS VM, document/render engine or browser framework dependency is used.
The pinned ryu-js numeric primitive supplies ECMAScript number formatting; it owns no VM state.
V9's scanner/source cursor, typed UTF-16/binary64 values, precedence AST, flat object arena and
evaluation remain Rust modules owned here. Functions pair parsed code-site IDs with defining
environment IDs; activations follow lexical parents and per-iteration let records preserve
captures. Private root var/function cells stay separate from lexical globals. Source schemas validate
every retained graph record before events; a finish-only iterative tracer reclaims unreachable
objects, functions and environments, including cycles, without moving identities. This is an owned
bounded collector, not V8's GC. Within-request allocation caps remain explicit failures.
Ordinary/arrow kind is AST/registry metadata; wire functions cannot choose it. Arrow
heads are recognized at assignment precedence using a bounded parenthesis table,
and concise bodies become budgeted returns. Both forms reuse the lexical graph,
activation and collector. Each parsed classic Program owns strictness; function code
inherits its defining mode and exact raw directive spelling can raise it. Prepared member
calls retain receivers before argument callbacks run. Arrows serialize a lexical receiver;
invocation environments retain effective receivers. Both are validated and traced across
all arenas. One always-rooted page-global object holds own data; global var/function properties
route to the existing root function cells, while lexical globals stay separate. Protected
global names fail explicitly; no native authority is exposed. Primitive wrappers and implicit
arguments objects remain unimplemented; explicit arguments cells retain lexical lookup.
Callable records distinguish source closures, bound targets and five canonical owned
intrinsics. All retain bounded own properties; fixed function-prototype lookup keeps
method identity and shadowing coherent. Invocation charges the same depth/work budget
for source and intrinsic calls. Bound chains remain flat IDs with explicit cycle/depth,
argument and metadata validation. Function properties and bound receiver/argument edges
participate in validation, quotas and finish-only tracing; canonical intrinsics remain
roots. Source display names stay separate from private recursive expression names.
A shared source walker collects var declarations across supported statements but stops
at nested functions. Script instantiation initializes object-environment cells only
for the script being reached; function activation initializes shared parameter,
var and direct-function cells before statements. Lexical blocks retain their own TDZ
schemas. Source validation requires initialized Var cells, including retained
activations, and rejects duplicate physical global mirrors. Initializers execute
in order; redeclarations without an initializer do not overwrite existing values.
Expression assignments, compounds and numeric updates share one synchronous
prepared reference over a resolved cell or retained receiver/key. No executable
reference is serialized. Ordinary stores preserve their existing error order;
compounds read before their RHS, while postfix returns the converted old Number.
Conditional branches are lazy and `typeof` handles only unresolved ordinary
identifiers specially. The parser and registry still inspect every source child;
call argument nesting contributes to the same expression depth bound. Named
handler calls and dedicated DOM text-write statements keep their existing seam.
Typed private state preserves numeric bits, including
nonfinite values and negative zero, plus stable object identity through page events. Script
locations refer to original UTF-8 source using UTF-16 columns. See
[UPSTREAM.md](UPSTREAM.md) for pinned reference code, observed routines, adaptations and limits.

Repository adaptations:

- Cargo tests Rust public surfaces. Unit tests mirror source domains; integration tests use
  real services. Synthetic loopback fixtures are the integration exception to no-network unit
  tests. An explicitly ignored GUI test opens only our native app.
- Rust constants hold limits. Exhaustive matches handle AST/syntax/control enums; operation
  dispatch remains explicit. Helpers target short logical steps, although rustfmt and FFI
  signatures add physical lines. Native adapter setup and atomic transactions remain coherent.
- NSAttributedString exact font metrics and primitive drawing replace the proposed direct
  CoreText calls. Measurement and painting share fixed attributes; Oya owns line breaks and
  positions. This avoids a second text stack without delegating page layout.
- The AppKit event loop polls worker updates at 16 ms. Agent descriptors are nonblocking, poll
  at 25 ms, restore their flags on drop, and never execute page code on the UI thread.

No release, existing engine replacement, schema migration or cloud deployment is performed.
The default Keychain item remains until cleared from Oya Native; uninstalling the package does not
remove that saved session.
