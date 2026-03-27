## 2024-03-27 - [HIGH] Fix timing attack vulnerability in token validation
**Vulnerability:** A timing attack vulnerability was identified in `handle_runtime_service_http_request` where string equality operators (`==` or `!=`) were used to compare HTTP service tokens.
**Learning:** For straightforward security algorithms like constant-time string comparisons, the project prefers implementing simple inline helper functions (e.g., using bitwise XOR) over adding external cryptographic dependencies like the `subtle` crate. This keeps the dependency footprint small while maintaining security.
**Prevention:** Always use a constant-time equality function (like `constant_time_eq`) when comparing sensitive data like tokens, passwords, or keys to prevent timing attacks.
