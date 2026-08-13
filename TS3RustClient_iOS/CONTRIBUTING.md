# Contributing to TS3Rust Client for iOS

## Code style

- Swift: SwiftLint with 4-space indentation; keep files small and focused.
- Rust: `cargo fmt` + `cargo clippy` before pushing (`rust_lib/`).
- All Rust log callbacks must be thread-safe and must not touch UI directly.
- FFI boundary: keep `rust_lib/src/lib.rs` minimal; add new exports with
  `#[no_mangle]` + regenerated `ts3_rust.h` (via `cbindgen`).

## Pull requests

- Must pass CI (`.github/workflows/build.yml`).
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org):
  `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`.
- If the public C API changes, regenerate and commit `rust_lib/include/ts3_rust.h`
  and the copy under `TS3Client/Resources/libs/`.

## Reporting bugs

Include: iOS version, device model, Xcode version, Rust version, server
address (or a local test server), steps to reproduce, and the on-screen log
view content (or crash log from Analytics Data).
