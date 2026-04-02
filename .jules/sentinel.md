## 2024-05-18 - Auth Token Timing Attack Vulnerability
**Vulnerability:** The HTTP runtime service used standard string equality (`==` or `!=`) to compare incoming HTTP bearer tokens against the expected service token.
**Learning:** This exposes the endpoint to a timing attack, where an attacker could deduce the token character-by-character based on the time taken to reject the request, as standard equality checks short-circuit on the first mismatched character.
**Prevention:** Always use constant-time comparison operations (e.g., bitwise XOR combined with `std::hint::black_box` to prevent compiler optimizations) when validating authentication tokens, secrets, or passwords.
