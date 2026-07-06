# devdeck

DevDeck — a Rust/egui desktop "Developer Workspace Manager". A fast hub for
AI-driven multi-repo development: judge in seconds whether a repo is safe to
work on (branch / pull needed / uncommitted), then open it in VS Code, a
terminal, or an AI agent.

## Commands

```powershell
cargo build                 # debug build
cargo test                  # unit tests
cargo run                   # launch the GUI
cargo fmt                   # format (CI enforces --check)
cargo clippy -- -D warnings # lint (CI enforces)
cargo install --path .      # install the `devdeck` CLI locally
```

## Architecture

| Module | Responsibility |
|---|---|
| `src/app.rs` | egui UI and application state (`DevDeckApp`) |
| `src/models.rs` | domain types (`Project`, `Preset`, `Settings`, `GitInfo`, `Config`) |
| `src/git.rs` | git CLI integration (`status --porcelain=v2 --branch`, fetch/pull/switch) |
| `src/actions.rs` | external launches (VS Code, terminal, agent, explorer) |
| `src/storage.rs` | JSON persistence (`%APPDATA%\devdeck\config.json`) |
| `src/theme.rs` | dark theme, status chips, primary button |
| `src/update.rs` | self-update against GitHub Releases |

Long-running work (git calls, update check/download) always runs on background
threads and reports back over the `mpsc` channel drained in `App::update` —
never block the UI thread.

## Rules

- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`).
- Before committing: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`
  must all pass (CI runs the same three).
- Releases follow `.claude/rules/release-process.md` — never create GitHub
  releases or tags by hand without the version-bump commit, and never rename
  release assets (the self-updater downloads `devdeck.exe` by exact name).
- egui's bundled fonts miss many glyphs (⎇ 🤖 ⧉ render as tofu boxes). When
  adding icons, stick to glyphs already used in the codebase or verify
  rendering visually before committing.
- Shell out to the `git` CLI for git operations (credential helpers work for
  free); do not introduce libgit2.
