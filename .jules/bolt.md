## 2024-06-18 - Unbounded Allocation in Log Tailing
**Learning:** `print_last_lines` in `loom-cli` allocated memory proportional to total file length by doing `.collect::<Vec<_>>()` on lines before tailing.
**Action:** Always favor iterator adapters like `.rev().take(n)` over `.collect()` when grabbing end slices of files to prevent unbounded memory growth.

## 2024-06-18 - Hardcoded Test Paths in wasm_runner
**Learning:** `wasm_runner` tests failed due to hardcoded `/home/ubuntu/...` absolute paths for workspace root.
**Action:** When working on tests, use `#[cfg(test)] std::env::temp_dir()` to avoid CI and permission issues across environments.
