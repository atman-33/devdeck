# DevDeck

**Developer Workspace Manager** — a fast hub for AI-driven, multi-repo development.

DevDeck is not just a VS Code launcher. In a development style where AI agents do
the implementation and humans check in to review, judge, and fix, DevDeck is the
human's home base: within seconds you can see which repositories are safe to work
on, open the ones you need, and resume where you left off.

![screenshot placeholder](docs/screenshot.png)

## Features

- **Project registry** — register local folders, filter by name/tag, favorites, recent
- **Git status at a glance** — current branch, ahead/behind upstream (pull needed?),
  uncommitted changes (work in progress?), refreshed in the background
- **Open in VS Code** — select any number of projects and open them as a single
  multi-root workspace (single selection opens the plain folder)
- **Workspace presets** — save/load named selections of projects
- **Git operations** — Fetch / Pull (`--ff-only`) / branch switch per project
- **Launchers** — terminal, Windows Explorer, and an AI agent (Claude Code by
  default) in the project directory; all commands are customizable in Settings
- **Per-project notes & tags**
- **Session restore** — the projects selected when you quit are selected again on
  next launch

## Install

Runtime requirements: `git` and the VS Code `code` command on PATH.

### From GitHub Releases (recommended)

```powershell
$dir = "$env:LOCALAPPDATA\Programs\devdeck"
New-Item -ItemType Directory -Force $dir | Out-Null
Invoke-WebRequest "https://github.com/atman-33/devdeck/releases/latest/download/devdeck.exe" -OutFile "$dir\devdeck.exe"
[Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";$dir", "User")
```

Open a new terminal and run:

```powershell
devdeck
```

Each release also ships `devdeck-windows-x86_64.zip` (exe + README + LICENSE)
and `SHA256SUMS.txt` if you prefer manual installation.

### From source

Requires Rust and Node.js 22+:

```powershell
npm install
npx tauri build --no-bundle
# -> src-tauri/target/release/devdeck.exe
```

### Updating

DevDeck checks GitHub Releases on startup. When a newer version exists, a
banner appears at the top — click **Update & restart** and it replaces itself
with the new version. The check can be disabled in `⚙ Settings`.

## Usage

1. **➕ Add projects** — pick one or more local repository folders.
2. Check the status badges per project:
   - `⎇ branch` — current branch
   - `↓N pull needed` — remote has commits you don't; pull before starting work
   - `↑N` — local commits not pushed
   - `● N changes` — uncommitted changes; the repo is mid-work
   - `clean` / `✓ up to date` — safe to start
3. Select projects with the checkboxes and hit **🚀 Open in VS Code**.
4. Save the current selection as a **preset** to reopen the same set later.

Ahead/behind counts compare against the last-fetched remote state; press
**⬇ Fetch all** (or per-project **Fetch**) to update them.

### Settings

`⚙ Settings` lets you customize the external commands (`{path}` is replaced with
the project path):

| Setting | Default |
|---|---|
| VS Code command | `code` |
| Terminal command | `wt -d {path}` |
| AI agent command | `wt -d {path} pwsh -NoExit -Command claude` |

## Data

Everything is stored in a single human-readable JSON file:
`%APPDATA%\devdeck\config.json`. Generated multi-root workspace files live in
`%USERPROFILE%\.devdeck\workspaces\` (not under AppData — some antivirus
folder shielding prevents a running VS Code instance from reading workspace
files there, which makes them open as an empty editor tab instead).

## Architecture

Tauri 2: Rust backend + React frontend in a single `devdeck.exe`.

Backend (`src-tauri/src/`):

| Module | Responsibility |
|---|---|
| `commands.rs` | `#[tauri::command]` layer exposed to the frontend |
| `models.rs` | domain types (`Project`, `Preset`, `Settings`, `GitInfo`, `Config`) |
| `git.rs` | git CLI integration (`status --porcelain=v2 --branch`, fetch/pull/switch) |
| `actions.rs` | external launches (VS Code, terminal, agent, explorer) |
| `storage.rs` | JSON persistence |
| `update.rs` | self-update against GitHub Releases |

Frontend (`src/`): React 19, Tailwind CSS v4, shadcn/ui, lucide icons.

Design notes:

- **Git**: shells out to the `git` CLI instead of libgit2, so fetch/pull reuse
  your existing credential helpers with zero auth configuration. All git calls
  run on background threads; the UI never blocks.
- **Extensibility**: launchers are template strings, so adding another agent or
  tool is a settings change, not a code change. The config file is versioned for
  future migrations.

## Development

```powershell
npm install
npm run tauri dev          # app with hot reload
cd src-tauri; cargo test   # backend unit tests
```

## License

MIT
