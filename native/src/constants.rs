/** Explicit resource budgets for the native Rust browser runtime. */
use std::time::Duration;

/// Wire compatibility is independent of the package version.
pub const VERSION: u32 = 2;
/// Includes JSON escaping and the terminating newline.
pub const FRAME_BYTES: usize = 256 * 1024;
pub const VM_STATE_BYTES: usize = FRAME_BYTES / 2;
/// Origin storage has an independent bounded serialized profile record.
pub const WEB_STORAGE_PROFILE_BYTES: usize = 512 * 1024;
/// A bounded encrypted record stores at most the active native tab session.
pub const SESSION_PROFILE_BYTES: usize = 64 * 1024;
/// Versioned tab-session records reject incompatible restore semantics.
pub const SESSION_PROFILE_VERSION: u32 = 1;
/// Versioned secure Web Storage records reject unknown future semantics.
pub const WEB_STORAGE_PROFILE_VERSION: u32 = 1;
/// A single Web Storage value remains small enough for a bounded page candidate.
pub const STORAGE_VALUE_BYTES: usize = 16 * 1024;
/// Each origin area has a small explicit entry ceiling.
pub const STORAGE_ENTRIES: usize = 256;
/// Each local or session area is bounded independently.
pub const STORAGE_AREA_BYTES: usize = 64 * 1024;
/// Limits a fetched page before parsing or interpretation.
pub const HTML_BYTES: usize = 64 * 1024;
/// Image transfers remain small enough for one bounded private JSON frame.
pub const IMAGE_BYTES: usize = 128 * 1024;
/// A committed page owns a bounded number of static image resources.
pub const IMAGE_COUNT: usize = 16;
/// Decoded image storage cannot exceed the shared snapshot budget.
pub const IMAGE_DECODED_BYTES: usize = 8 * 1024 * 1024;
/// Image dimensions prevent pathological decoder allocations.
pub const IMAGE_DIMENSION: u32 = 2048;
/// Table grids stay comfortably below document and display-list caps.
pub const TABLE_ROWS: usize = 64;
/// A wide grid cannot consume the full node budget with empty cells.
pub const TABLE_COLUMNS: usize = 16;
/// Cell allocations remain independently bounded per table.
pub const TABLE_CELLS: usize = TABLE_ROWS * TABLE_COLUMNS;
/// Headers have a separate budget from the body.
pub const HEADER_BYTES: usize = 8 * 1024;
/// Bounds snapshots and layout work.
pub const NODES: usize = 512;
/// Applies to parsed and replaced node text alike.
pub const TEXT_BYTES: usize = 2 * 1024;
/// Limits source before tokenization.
pub const SCRIPT_BYTES: usize = 4 * 1024;
/// Bounds the number of executable scripts accepted in one candidate document.
pub const SCRIPT_COUNT: usize = 8;
/// One page-level V9 program cannot multiply parser and VM allocation without bound.
pub const SCRIPT_TOTAL_BYTES: usize = SCRIPT_BYTES * SCRIPT_COUNT;
/// Bounds the interpreter environment and proposed mutations.
pub const STATEMENTS: usize = 64;
/// Bounds tokenizer allocation.
pub const TOKENS: usize = 1024;
/// Bounds interpreter work even as the grammar grows.
pub const OPERATIONS: usize = 2048;
/// Numeric update operators add or subtract one after supported primitive conversion.
pub const JS_UPDATE_STEP: f64 = 1.0;
/// Limits control input before URL parsing.
pub const URL_BYTES: usize = 2 * 1024;
/// Limits response correlation identifiers.
pub const ID_BYTES: usize = 128;
/// Absolute deadline for a complete HTTP exchange.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(2);
/// Covers child writes and reads, not just response reads.
pub const RPC_TIMEOUT: Duration = Duration::from_secs(3);
/// Successful status supported by the minimal HTTP implementation.
pub const HTTP_OK: u16 = 200;
/// Protocol separator for HTTP headers and body.
pub const HEADER_END: &[u8] = b"\r\n\r\n";
/// Small socket reads keep transient allocation below the complete response budget.
pub const NETWORK_CHUNK_BYTES: usize = 1024;
/// Native layout coordinates count Unicode scalars, not pixels.
pub const LAYOUT_UNIT: &str = "unicode_scalar_cells";
/// Desktop web requests include TLS and redirects within one absolute budget.
pub const WEB_TIMEOUT: Duration = Duration::from_secs(8);
/// The external supervisor also bounds uncancellable operating-system DNS calls.
pub const WEB_RPC_TIMEOUT: Duration = Duration::from_secs(10);
/// Redirects must revalidate destinations and cannot create an unbounded chain.
pub const REDIRECTS: usize = 5;
/// Excessive or mixed DNS answer sets are rejected rather than partially trusted.
pub const DNS_ANSWERS: usize = 16;
/// Explicit supported markup depth prevents recursive parser/layout exhaustion.
pub const DOCUMENT_DEPTH: usize = 32;
/// Committed history is a bounded local session resource.
pub const HISTORY_ENTRIES: usize = 64;
/// Native windows cap retained page documents to bound aggregate tab memory.
pub const TABS: usize = 16;
/// Cancellation stays responsive even while a worker waits for a child reply.
pub const RPC_POLL: Duration = Duration::from_millis(25);
/// A child can detect coordinator death independently of a blocked engine dispatch.
pub const OWNER_PID_ENV: &str = "OYA_NATIVE_OWNER_PID";
/// Required resources share a single navigation budget.
pub const STYLE_BYTES: usize = 16 * 1024;
/// At most four required sheets can contribute to one candidate.
pub const STYLE_SHEETS: usize = 4;
/// Submission data is bounded independently of document source.
pub const FORM_BYTES: usize = 16 * 1024;
/// Session cookies have independent count and byte limits.
pub const COOKIE_COUNT: usize = 64;
/// One host cannot occupy the whole session jar.
pub const HOST_COOKIES: usize = 16;
/// A cookie cannot grow a request beyond the framing budget.
pub const COOKIE_BYTES: usize = 4096;
/// Aggregate jar allocation stays bounded even with many small origins.
pub const COOKIE_JAR_BYTES: usize = 32 * 1024;
/// Version and JSON framing fit beside the bounded cookie payload in Keychain.
pub const COOKIE_PROFILE_BYTES: usize = 48 * 1024;
/// A stuck OS Keychain request cannot hold the browser worker or shutdown indefinitely.
pub const PROFILE_STORE_TIMEOUT: Duration = Duration::from_secs(3);
/// Resource and compatibility reports never grow with arbitrary source features.
pub const DIAGNOSTICS: usize = 32;
/// Recursive parsing and AST evaluation share an independently checked depth bound.
pub const EXPRESSION_DEPTH: usize = 32;
/// Blocks and single-statement control bodies cannot exhaust the parser or evaluator stack.
pub const STATEMENT_DEPTH: usize = 32;
/// JavaScript strings count UTF-16 code units, independently of DOM UTF-8 bytes.
pub const JS_STRING_UNITS: usize = 2048;
/// A service-provided environment cannot multiply the per-string budget without bound.
pub const JS_ENVIRONMENT_UNITS: usize = 16384;
pub const VM_OBJECTS: usize = 256;
/** The first allocation is the page's permanent global receiver, never a replaceable supplied object. */
pub const VM_GLOBAL_OBJECT_ID: u32 = 0;
pub const VM_PROPERTIES: usize = 1024;
pub const VM_PROPERTY_KEY_UNITS: usize = 64;
pub const VM_ARRAY_LENGTH: usize = 1024;
pub const FUNCTION_PARAMETERS: usize = 16;
/// Retained lexical contexts and within-request allocations have explicit independent ceilings.
pub const VM_ENVIRONMENTS: usize = 512;
/// Closures are records rather than serialized executable callbacks.
pub const VM_FUNCTIONS: usize = 512;
/// Captured cells cannot multiply the per-variable string budget without bound.
pub const VM_BINDINGS: usize = 2048;
/// Function syntax nesting is bounded independently of calls and ordinary expression grouping.
pub const FUNCTION_NESTING: usize = 16;
/// Callable helpers cannot expand one invocation beyond the parser's argument ceiling.
pub const VM_CALL_ARGUMENTS: usize = FUNCTION_PARAMETERS;
/// Binding chains are checked iteratively before evaluation or private-state publication.
pub const VM_BOUND_HOPS: usize = FUNCTION_NESTING;
/// Canonical intrinsic identities are page-owned and cannot be selected by serialized code.
pub const VM_FUNCTION_CONSTRUCTOR_ID: u32 = 0;
pub const VM_FUNCTION_PROTOTYPE_ID: u32 = 1;
pub const VM_FUNCTION_CALL_ID: u32 = 2;
pub const VM_FUNCTION_APPLY_ID: u32 = 3;
pub const VM_FUNCTION_BIND_ID: u32 = 4;
pub const VM_FUNCTION_INTRINSICS: u32 = 5;
/// ToLength clamps to ECMAScript's maximum safe integer before the smaller invocation cap applies.
pub const VM_MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;
