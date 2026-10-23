<!-- Relay's independent adoption surface and explicit transport policy. -->
# Relay

Relay owns Mars's bounded HTTP/TLS implementation. Use it without starting the
Mars app, Eye, V9 or a service process:

```toml
[dependencies]
mars-relay = { path = "/path/to/agentchrome/mars/crates/relay" }
```

```rust
use mars_relay::{WebRequest, policy};

let request = WebRequest::get("https://1.1.1.1/document");
request.validate().expect("bounded GET");
let url = policy::web_url(&request.url).expect("HTTP/HTTPS URL syntax");
assert!(policy::public_address("1.1.1.1".parse().unwrap()));
assert_eq!(url.path(), "/document");
```

This example validates data without DNS or socket I/O. `web_url` checks syntax;
it does not by itself authorize a resolved destination. `Prepared::connect`
revalidates requests and URL policy, resolves and checks every address, and
connects to an exact admitted address, including normal TLS verification.
Consuming `Prepared::transmit` sends HTTP application bytes once. The caller
must perform its own application/agent ownership admission before that step.
Only exact explicitly granted loopback origins bypass the public-address
policy. No ambient proxy or credentials are used.

The existing bounded subset includes GET, URL-encoded POST, bounded HTTP
framing, host-only cookies, verified TLS and explicit redirects. It is under
development, is not a complete Chromium networking replacement, and is not
published to crates.io. Cookie profile records are pure bytes; persistence and
secure storage remain the caller's responsibility. Legacy `fetch`/`Endpoint`
are the separately restricted literal-loopback fixture API.

Run `cargo run --manifest-path mars/Cargo.toml -p mars-relay --example policy`
from the repository root for the no-network public consumer example.
