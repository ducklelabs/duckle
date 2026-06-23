//! Coarse error categorization for run results, alerts, and logs.
//!
//! Connector and DuckDB failures all reach the run report as strings
//! (`EngineError::Query(msg)` mostly), which makes programmatic routing -
//! "retry network errors, fail fast on schema errors", alert payloads,
//! log filtering in Splunk - impossible without parsing prose. This module
//! buckets an error message into a small, stable set of categories by
//! pattern-matching the text. The mapping is heuristic by design: it only
//! has to be useful, not perfect, and `other` is always a safe answer.
//!
//! Categories (stable, lowercase - external tooling may key on them):
//! `auth`, `network`, `timeout`, `oom`, `disk`, `schema`, `syntax`,
//! `cancelled`, `other`.

/// Bucket an error message into a coarse category. Order matters: the
/// most specific signals are tested first so e.g. a "connection timed
/// out" lands in `timeout`, not `network`.
pub fn categorize_error(msg: &str) -> &'static str {
    let m = msg.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| m.contains(n));

    if has(&["cancelled", "canceled by user"]) {
        return "cancelled";
    }
    if has(&["timed out", "timeout", "deadline exceeded"]) {
        return "timeout";
    }
    if has(&[
        "out of memory",
        "memory limit",
        "failed to allocate",
        "cannot allocate",
        "oom",
    ]) {
        return "oom";
    }
    if has(&[
        "no space left",
        "disk full",
        "too many open files",
        "quota exceeded",
    ]) {
        return "disk";
    }
    if has(&[
        "authentication",
        "unauthorized",
        "forbidden",
        "access denied",
        "permission denied",
        "login failed",
        "invalid credentials",
        "password authentication failed",
        "jwt token is invalid",
        "token expired",
        "http 401",
        "http 403",
        "status 401",
        "status 403",
        "(401)",
        "(403)",
    ]) {
        return "auth";
    }
    if has(&[
        "connection refused",
        "connection reset",
        "connection closed",
        "could not connect",
        "failed to connect",
        "unable to connect",
        "broken pipe",
        "no such host",
        "name resolution",
        "dns error",
        "network unreachable",
        "host unreachable",
        "tls handshake",
        "certificate",
        "http 502",
        "http 503",
        "http 504",
        "status 502",
        "status 503",
        "status 504",
        "transport error",
        "io error",
    ]) {
        return "network";
    }
    // DuckDB prefixes reference/type problems as Binder / Catalog /
    // Conversion errors; connectors add their own "column ... not found"
    // style messages. All are schema-shaped: the data or references no
    // longer match what the pipeline expects.
    if has(&["syntax error", "parser error", "parse error"]) {
        return "syntax";
    }
    if has(&[
        "binder error",
        "catalog error",
        "conversion error",
        "could not convert",
        "cannot cast",
        "type mismatch",
        "schema mismatch",
        "does not exist",
        "not found in",
        "no such column",
        "no such table",
        "unknown column",
        "invalid input error",
    ]) {
        return "schema";
    }
    "other"
}

#[cfg(test)]
mod tests {
    use super::categorize_error;

    #[test]
    fn buckets_common_messages() {
        let cases = [
            ("Connection refused (os error 111)", "network"),
            ("TLS handshake failed: bad certificate", "network"),
            ("connection timed out after 30000 ms", "timeout"),
            ("Out of Memory Error: failed to allocate block", "oom"),
            ("IO Error: No space left on device", "disk"),
            ("snowflake: JWT token is invalid (390144)", "auth"),
            (
                "ORA-01017: invalid username/password; logon denied. authentication failed",
                "auth",
            ),
            ("Binder Error: column \"order_id\" not found", "schema"),
            ("Catalog Error: Table 'orders' does not exist", "schema"),
            (
                "Conversion Error: Could not convert string 'abc' to INT64",
                "schema",
            ),
            ("Parser Error: syntax error at or near \"SELEC\"", "syntax"),
            ("run was cancelled", "cancelled"),
            ("something inexplicable happened", "other"),
        ];
        for (msg, want) in cases {
            assert_eq!(categorize_error(msg), want, "msg: {msg}");
        }
    }

    #[test]
    fn timeout_beats_network() {
        // "connection timed out" mentions both; timeout is the actionable bucket.
        assert_eq!(categorize_error("connection timed out"), "timeout");
    }
}
