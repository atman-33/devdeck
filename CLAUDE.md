# devdeck

DevDeck — a Tauri 2 desktop "Developer Workspace Manager" (Rust backend +
React/TypeScript/shadcn-ui frontend). A fast hub for AI-driven multi-repo
development: judge in seconds whether a repo is safe to work on (branch /
pull needed / uncommitted), then open it in VS Code, a terminal, or an AI
agent.

## Commands

```powershell
npm install                # frontend dependencies (first time)
npm run tauri dev          # run the app with hot reload
npx tauri build --no-bundle # release build -> src-tauri/target/release/devdeck.exe
npm run build              # typecheck + build frontend only

# in src-tauri/
cargo test                 # unit tests (on the local windows-gnu toolchain use
                           #   `cargo test --release` — debug test exes fail to
                           #   load with STATUS_ENTRYPOINT_NOT_FOUND)
cargo fmt                  # format (CI enforces --check)
cargo clippy -- -D warnings # lint (CI enforces)
```

## Architecture

Backend (`src-tauri/src/`):

| Module | Responsibility |
|---|---|
| `lib.rs` / `main.rs` | Tauri builder, command registration |
| `commands.rs` | `#[tauri::command]` layer exposed to the frontend |
| `models.rs` | domain types (`Project`, `Preset`, `Settings`, `GitInfo`, `Config`) |
| `git.rs` | git CLI integration (`status --porcelain=v2 --branch`, fetch/pull/switch) |
| `actions.rs` | external launches (VS Code, terminal, agent, explorer) |
| `storage.rs` | JSON persistence (`%APPDATA%\devdeck\config.json`) |
| `update.rs` | self-update against GitHub Releases |

Frontend (`src/`): React 19 + Tailwind v4 + shadcn/ui. `App.tsx` owns all
state and persistence; `components/ProjectRow.tsx` renders one compact row;
`lib/api.ts` wraps `invoke()` with types matching the Rust structs
(snake_case fields; command *parameters* are camelCase — Tauri converts).

Blocking work (git calls, update download) runs via
`tauri::async_runtime::spawn_blocking` in commands — never on the UI thread.

## Rules

- Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`).
- Before committing: `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test --release`, and `npm run build` (typecheck) must pass.
- Releases follow `.claude/rules/release-process.md` — never create GitHub
  releases or tags by hand without the version-bump commit, and never rename
  release assets (the self-updater downloads `devdeck.exe` by exact name).
- Config compatibility: `%APPDATA%\devdeck\config.json` is read by every
  released version — only add fields with `#[serde(default)]`, never rename
  or remove existing ones.
- Shell out to the `git` CLI for git operations (credential helpers work for
  free); do not introduce libgit2.
- Keep `crate-type` in `src-tauri/Cargo.toml` as plain rlib (no cdylib) —
  cdylib breaks windows-gnu debug builds ("export ordinal too large").
