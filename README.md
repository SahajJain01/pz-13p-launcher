<div align="center">

# PZ 13th Pandemic Launcher

<p>A polished Windows launcher for the Project Zomboid "13th Pandemic" modpack. Detect, link, optimize, and play in one click.</p>

<p>
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white&style=for-the-badge" />
  <img alt="React" src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white&style=for-the-badge" />
  <img alt="TypeScript" src="https://img.shields.io/badge/TypeScript-5.x-3178C6?logo=typescript&logoColor=white&style=for-the-badge" />
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Edition%202021-000000?logo=rust&logoColor=white&style=for-the-badge" />
  <img alt="Vite" src="https://img.shields.io/badge/Vite-7-646CFF?logo=vite&logoColor=white&style=for-the-badge" />
  <img alt="Windows" src="https://img.shields.io/badge/Windows-10%2F11-0078D4?logo=windows&logoColor=white&style=for-the-badge" />
  <img alt="Steam" src="https://img.shields.io/badge/Steam-Required-000000?logo=steam&logoColor=white&style=for-the-badge" />
  <br/>
  <img alt="Status" src="https://img.shields.io/badge/Platform-Windows%20only-informational?style=flat-square" />
  <img alt="PZ" src="https://img.shields.io/badge/Game-Project%20Zomboid-orange?style=flat-square" />
</p>

</div>

---

## Highlights

- Smart auto‑detect of Steam and Workshop locations
- One‑click “Download” opens the Workshop mod page in Steam
- One‑click “Play” launches PZ with the correct `-cachedir`
- Safe mod linking: creates junctions and backs up your original mods
- Apply Optimizations: copies performance files into the PZ install folder
- Built‑in logging and a minimal settings panel

> Windows only. Tested with Steam installs and Workshop ID 3487726294.

## Quick Start

1) Subscribe to the 13th Pandemic Workshop mod (or click “Download” in the app).
2) Click “Refresh” once Steam finishes downloading the mod.
3) Optional: open Settings → “Apply Optimizations” to copy extra performance files into your PZ install (skips if already applied).
4) Click “Play”. The launcher starts Steam (if needed), sets `-cachedir` to the mod’s Zomboid folder, and joins the server.

## How It Works

- Detects Steam via registry, finds your Workshop libraries, and locates the mod folder.
- Links submods into your user mods directory using directory junctions.
- Launches PZ through Steam with `-cachedir` pointing to the pseudo mod’s Zomboid folder.
- On cleanup, removes junctions and restores any backups.
- Optimizations (optional): copies everything from `<workshop>\mods\13thPandemic\ProjectZomboid` into `...\steamapps\common\ProjectZomboid` and remembers if already applied (by file sizes).

## Screenshots

<!-- Drop your images into /public or /docs and update paths below -->
<!-- ![Launcher home](docs/screenshot-home.png) -->
<!-- ![Settings panel](docs/screenshot-settings.png) -->

## Development

### Prerequisites

- Node.js (18+) or [Bun](https://bun.sh/)
- Rust (stable), Cargo
- Windows build tools (MSVC) and WebView2
- [Tauri CLI](https://tauri.app/) 2.x

### Install dependencies

```bash
bun install   # or npm install
```

### Run in development

```bash
# Frontend dev server
bun run dev          # or npm run dev

# Tauri shell
bun run tauri dev    # or npm run tauri dev
```

### Build

```bash
bun run build        # or npm run build
bun run tauri build  # or npm run tauri build
```

### Lint & Test

```bash
bun run lint         # or npm run lint
bun test             # or npm test
```

## Troubleshooting

- “Mod not found” → Click “Download” to open Steam; subscribe and wait for download, then press “Refresh”.
- “Optimizations already applied” → The optimization files in your game folder match the workshop versions. No action needed.
- Symlink/junction errors → Run the launcher as Administrator and ensure antivirus isn’t blocking junction creation.
- Steam not detected → Make sure Steam is installed and has run at least once on this account.

## Contributing

PRs welcome! Please run lint, tests, and `cargo check` before submitting. Keep changes focused and consistent with the current style.

