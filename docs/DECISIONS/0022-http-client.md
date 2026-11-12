# ADR 0022: HTTP client for `http.fetch`

- Status: accepted
- Date: 2026-11-10

## Context

`http.fetch` is synchronous in the WIT and allowed only in async instances (worker threads) for
plugins with `capabilities.network = ["domain", "*.domain"]`. The host needs a blocking client
with timeouts, a response size cap and TLS, small enough not to drag an async runtime into the
host.

## Decision

**`ureq` 3.5** (MIT OR Apache-2.0), features `rustls` (ring provider, webpki roots) and `gzip`.
One shared `Agent` with `max_redirects(0)` and `http_status_as_error(false)`.

Checks before any I/O: the URL parses, scheme is `https` (or `http` when
`HttpConfig::allow_plain_http`), no credentials in the authority, the **host** is allowed by
`Capabilities::network_allows` (exact names, `*.suffix` for subdomains), method is a valid
token, request body ≤ `max_request_bytes` (8 MiB). Timeout = `timeout-ms` (0 → 10 s) capped by
`max_timeout` (60 s); response body read with a limit (`max_response_bytes`, 8 MiB). Redirects
are **not** followed: a 3xx is returned to the plugin, so a redirect can never leave the
allowlist. Errors map to `host-error`: `capability-denied`, `invalid`, `other` (timeouts, size,
transport). Time blocked in the request does not count against the async call's deadline.

## Consequences

- Transitive dependencies: rustls, ring (Apache-2.0 AND ISC), webpki-roots
  (CDLA-Permissive-2.0), flate2; all allowed by `deny.toml`.
- DNS is not pinned: an allowed name resolving to a private address is reachable (the admin
  chose the domain). A resolver-level filter can be added later without API change.
- No HTTP/2, no connection-level proxy configuration beyond ureq's environment handling.

## Alternatives considered

- **reqwest (blocking)**: pulls tokio and hyper into the host for a feature used off-tick only.
- **wasi:http**: plugins would need a different, larger interface; the allowlist would have to be
  enforced in an outgoing-handler hook anyway. Possible later as an additive import.
