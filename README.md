# DataZen Native

Native desktop database client rebuilt around a Rust workspace:

- `backend/` contains storage, drivers, connection management, query execution, and the background request gateway.
- `frontend/` contains a native GPUI application with its own state, error handling, and i18n layer.
- `src/main.rs` builds a single executable that links both halves together.

## Goals

- Zero JavaScript in the active application path.
- Native frontend with GPUI.
- Backend and UI fully separated through a request gateway so transport or driver failures do not crash the window.
- Safer defaults: the old `Kiwi` HTTP integration is removed from the active build, and SSH tunneling is disabled until strict host-key verification is implemented.
- Localized UI with:
  - English
  - Spanish
  - Portuguese
  - French
  - German
  - Russian
  - Ukrainian
  - Mandarin Chinese
  - Hindi
  - Japanese
  - Korean

## Build

```bash
cargo run
```

Release build:

```bash
cargo build --release
```

The root package produces one executable: `datazen-native`.

## Layout

```text
datazen/
├── backend/
│   └── src/
├── frontend/
│   └── src/
├── src/
│   └── main.rs
└── Cargo.toml
```

## Notes

- The current native shell covers connection management, connection testing, query execution, table preview, and runtime language switching.
- Release size is optimized in `Cargo.toml` with `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, and `strip = true`.
- SSH support is intentionally blocked for now because the old implementation accepted any host key, which is not acceptable for a hardened rewrite.
