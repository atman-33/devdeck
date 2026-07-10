# Changelog

## v0.4.0 — 2026-07-10

- Project rows: "Copy path" action (dropdown item + inline icon next to the
  path) copies the project's full path to the clipboard
- Project rows: "Open on GitHub" dropdown action opens the repo's remote URL
  (derived from `git remote get-url origin`, normalized to `https://`) in
  the default browser

## v0.3.0 — 2026-07-07

- UI rebuilt on Tauri 2 + React + shadcn/ui: compact one-row project list
  (see many repos at once), hover quick actions, per-row `⋯` menu
  (terminal / agent / explorer / fetch / pull / branch switch / notes),
  refined dark theme, tooltips everywhere
- Same Rust core (git CLI integration, storage, self-update) exposed as Tauri
  commands; config file location and format unchanged — settings carry over
- Self-update keeps the same asset contract, so 0.2.x installs update
  seamlessly via the in-app banner

## v0.2.1 — 2026-07-07

- Fix: launching VS Code no longer leaves a lingering console window open
  (dropped `cmd /C start` in favor of a hidden console; VS Code's code.cmd
  shim kept the started console alive until VS Code exited)

## v0.2.0 — 2026-07-06

- Self-update: on startup DevDeck checks GitHub Releases and shows an in-app
  banner when a newer version exists; one click downloads, swaps the exe, and
  offers a restart (toggle in Settings)
- Versioning & release automation: CI (fmt/clippy/test/build) and a tag-driven
  release workflow that publishes `devdeck.exe`, `devdeck-windows-x86_64.zip`,
  and `SHA256SUMS.txt`
- README installation instructions based on GitHub Releases

## v0.1.0 — 2026-07-06

- Initial release: project registry, git status chips (branch / ahead-behind /
  uncommitted), multi-root VS Code launch, workspace presets, tags/favorites/
  recent, per-project notes, terminal/agent/explorer launchers, session restore
- Modern dark UI (custom egui theme, card layout, status chips)
