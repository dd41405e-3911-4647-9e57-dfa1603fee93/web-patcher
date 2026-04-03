<p align="center">
  <img src="assets/icons/icon-256.png" alt="OWTK Patcher" width="256">
</p>

<p align="center">A web-based firmware patching tool for OneWheels, built in Rust and compiled to WebAssembly.</p>

<p align="center">Installable as a PWA — also works on mobile!</p>

<p align="center"><a href="https://patcher.owtk.dev/">Try it out</a></p>

## Supported Boards

| MCU Family | Boards |
|---|---|
| **STM32F103** (F1) | XR, Pint, PintX |
| **STM32F407** (F4) | GT, GTS, XRC |

## Features

### Firmware Patching

Load firmware images, apply patches through a visual interface, and save the result. Patches are written as declarative [Rhai](https://rhai.rs) scripts with per-board/per-version targeting and dynamic UI parameters (toggles, sliders, dropdowns).

**Available firmware patches:**

| Patch | Description |
|---|---|
| **Modify Top Speed** | Per-mode speed limit adjustment (available modes vary by firmware) |
| **Bypass BLE Auth** | Skip Bluetooth authentication |
| **Disable Battery Type Check** | Remove the controller's battery type validation |
| **Disable Speed Haptic** | Disable haptic alerts for speed, torque, and error conditions |
| **Report Voltages** | Restore battery and cell voltage reporting over their original BLE characteristics |
| **Spoof Generation** | Override the hardware revision reported to third party apps |
| **Third Party Battery** | Remove third party battery pack restrictions and recalculate SoC to reflect actual battery percentage |
| **Unlock FWU** | Remove restrictions on entering firmware update mode |

**Available bootloader patches:**

| Patch | Description |
|---|---|
| **Bypass Flashing Restrictions** | Skip the RSA signature check and firmware version comparison performed on OTA files |

### Encryption & Decryption

Supports three firmware encryption schemes:

- AES-128-ECB (older F1 boards)
- AES-128-CTR with static IV (GT v1-v2)
- AES-128-CTR with dynamic IV (GT v3, GTS, XRC)

Encryption keys can be extracted from full flash dumps or bootloader files.

### Flash Backup Management

Parse full flash backups (F1: 64KB, F4: 1MB) to:

- Extract, modify, and replace firmware and bootloader images
- View and edit board configuration (currently limited to core fields like BMS settings and IMU calibration, with more planned if needed)
- Modify bootloader version
- Rebuild and save modified backups

### Firmware Identification

Automatic identification of known firmware versions via SHA-1 hash matching (full and partial). Trial decryption with loaded keys for unrecognized images.

## Project Structure

```
owtk-patcher/
├── crates/owtk-core/       Core library (firmware, crypto, patches, backup parsing)
│   └── src/
│       ├── firmware/        Firmware registry and identification
│       ├── bootloader/      Bootloader registry and identification
│       ├── crypto/          AES encryption/decryption, key extraction
│       ├── patches/         Patch system, Rhai scripting engine
│       │   └── scripts/     Patch scripts (.rhai) and scripting docs
│       └── backup/          Flash backup parsing and config management
├── src/                     Web application (egui/eframe)
│   ├── app/                 Application state and file loading
│   └── ui/                  UI components (firmware, bootloader, backup, keys)
└── .github/workflows/       CI/CD (test + deploy to Cloudflare Pages)
```

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (2024 edition)
- [Trunk](https://trunkrs.dev/) (for WASM builds)

### Run Tests

```bash
cargo test
```

### Build for Web

```bash
rustup target add wasm32-unknown-unknown
trunk build --release
```

The output is written to `dist/` and can be served as a static site.

### Development Server

```bash
trunk serve
```

## Writing Patches

Patches are Rhai scripts located in `crates/owtk-core/src/patches/scripts/`. Each script defines:

- **`patch()`** - Metadata, description, and per-firmware target offsets
- **`parameters()`** - UI controls for user-configurable values
- **`apply(params)`** - Byte writes to apply based on parameter values
- **`read(fw)`** *(optional)* - Read current values back from patched firmware

See [`SCRIPTING.md`](crates/owtk-core/src/patches/scripts/SCRIPTING.md) for the full scripting API reference.

## Tech Stack

- **Rust** - Core logic and web application
- **egui / eframe** - Immediate-mode UI framework
- **Rhai** - Embedded scripting for patch definitions
- **WebAssembly** - Browser deployment target
- **Trunk** - WASM build tooling
- **Cloudflare Pages** - Hosting
