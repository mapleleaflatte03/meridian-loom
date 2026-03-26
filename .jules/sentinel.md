## 2024-05-24 - [Timing Attack in Auth Token Comparison]
**Vulnerability:** Timing attack possible in `handle_runtime_service_http_request` where the HTTP authorization token was compared using standard inequality (`presented != expected`).
**Learning:** Even internal tool boundaries or development runtimes like `meridian-loom` can expose timing vulnerabilities if they serve requests over HTTP (even locally).
**Prevention:** Implement and use a `constant_time_eq` helper using bitwise operations for all sensitive token or credential comparisons.
