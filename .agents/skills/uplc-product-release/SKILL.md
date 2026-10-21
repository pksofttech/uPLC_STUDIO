---
name: uplc-product-release
description: >-
  Use this skill when compiling uPLC Studio (laduino-plc) as a distributable product,
  packaging release archives (.tar.gz / .zip) with bundled RP2040 .uf2 firmware and
  example programs, configuring the online auto-update system (updater.rs / self_update),
  or updating GitHub Actions release workflows (.github/workflows/release.yml).
---

# uPLC Studio Product Packaging & Online Update Skill

This skill defines the standard release pipeline for **uPLC Studio (`laduino-plc`)**, including the `self_update` auto-updater ([updater.rs](../../../crates/ui/src/updater.rs)) and the GitHub Actions CI/CD workflow ([release.yml](../../../.github/workflows/release.yml)).

## Product Bundle Structure

Every distributable product archive must follow the naming convention required by `self_update`:

- **Linux (`x86_64-unknown-linux-gnu`)**: `laduino-plc-v<VERSION>-x86_64-unknown-linux-gnu.tar.gz`
- **Windows (`x86_64-pc-windows-msvc`)**: `laduino-plc-v<VERSION>-x86_64-pc-windows-msvc.zip`

Inside the archive root:
```text
├── laduino-plc (or laduino-plc.exe)        # Main desktop IDE binary
├── firmware/
│   └── uplc-rp2040-embassy.uf2             # Pre-built RP2040 USB BOOTSEL firmware
└── example/
    └── *.il                                # 10 bundled IEC 61131-3 IL example programs
```

## Online Auto-Update Modes (`updater.rs`)

1. **GitHub Releases Mode (`use_custom_server = false`)**:
   - Queries `https://api.github.com/repos/<owner>/<repo>/releases` via `self_update::backends::github::ReleaseList`.
   - Downloads the matching target archive and atomically swaps `laduino-plc` via `self_update::backends::github::Update`.
2. **Private Server Mode (`use_custom_server = true`)**:
   - Downloads a `latest.json` manifest ([`CustomReleaseManifest`](../../../crates/ui/src/updater.rs)) from a custom HTTP/HTTPS URL:
     ```json
     {
       "version": "0.2.0",
       "notes": "- Release notes here",
       "platforms": {
         "x86_64-unknown-linux-gnu": {
           "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
         },
         "x86_64-pc-windows-msvc": {
           "url": "https://updates.example.com/laduino-plc-v0.2.0-x86_64-pc-windows-msvc.zip"
         }
       }
     }
     ```
3. **Industrial Safety Locks**:
   - [`AppUpdater::safety_block_reason`](../../../crates/ui/src/updater.rs) prevents installing updates or restarting while `app.online_mode == true`, `app.doc.simulator.is_active()`, or `app.doc.is_dirty == true`.

## Step-by-Step Local Release Packaging

1. Update `version` in [Cargo.toml](../../../Cargo.toml).
2. Run the packaging script [package_release.sh](./scripts/package_release.sh):
   ```bash
   bash .agents/skills/uplc-product-release/scripts/package_release.sh
   ```
3. Commit and push the version tag to trigger [release.yml](../../../.github/workflows/release.yml):
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
