## 2025-03-29 - [Fix timing attack vulnerability in HTTP auth]
**Vulnerability:** HTTP authentication token was validated using a basic string equality check (`presented != expected`), which fails early.
**Learning:** This introduces a timing attack vector. When verifying authentication tokens, it's vital to use a constant-time comparison to avoid leaking the correct string token character by character. We've introduced a helper function `constant_time_eq` utilizing bitwise XOR wrapped in `std::hint::black_box` to avoid compiler loop optimizations and guarantee constant execution time.
**Prevention:** Always implement or use established constant-time comparison primitives (like `constant_time_eq` or `subtle` crate) when verifying cryptographic credentials or authentication tokens.
