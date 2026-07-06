# Changelog

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
