# PZ 13th Pandemic Launcher

A desktop launcher for the Project Zomboid "13th Pandemic" modpack. It links the modpack from a single Steam Workshop item into your local mods folder and launches the game connected to the community server.

## Features

- Detects Steam and mod directories
- Opens the Workshop page for the required modpack
- Symlinks downloaded mods and restores backups on exit
- Debug panel for troubleshooting paths and logs

## Development

### Prerequisites

- Node.js or [Bun](https://bun.sh/)
- Rust and Cargo
- [Tauri CLI](https://tauri.app/)

### Install dependencies

```bash
bun install   # or npm install
```

### Run in development

```bash
bun run dev          # starts Vite dev server
bun run tauri dev    # launches the Tauri shell
```

### Build

```bash
bun run build
bun run tauri build
```

### Lint & Test

```bash
bun run lint
bun test
```

## Contributing

Pull requests are welcome. Please run lint, tests, and `cargo check` before submitting changes.

