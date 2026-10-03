# Plattypus Web Player & Arcade

This directory contains the Plattypus Web Arcade, powered by a customized build of the **PSoXide** WebAssembly PlayStation 1 emulator.

## Architecture

* **Frontend Arcade Shell (`index.html`, `app.js`, `style.css`):**
  * Provides the CRT arcade frame, responsive layout, and mobile touch overlay.
  * Embeds the PSoXide web player iframe (`player/?embed=1&disc=../roms/plattypus.cue`).
  * Emulates authentic DualShock / digital pad controls via pointer events and bridges button bitmasks to the Wasm player using `postMessage` and `_psoxideVirtualPadMask`.

* **Web Player Wasm Core (`player/`):**
  * Built using Rust, `wasm-bindgen`, and `Trunk` targeting `wasm32-unknown-unknown`.
  * Features HTTP range-request streaming (`206 Partial Content`) for instant startup of large multi-track disc images (`plattypus.bin`).
  * Runs hardware-accurate MIPS R3000A emulation, software GPU rendering with WebGL2 fallback, SPU audio synthesis with CD-DA Red Book audio streaming, and MDEC hardware video decoding for 15 fps STR cutscenes with interleaved CD-XA ADPCM audio.

## Upstream & Custom Emulator Source

The customized PSoXide emulator source is maintained in the personal fork:
**[https://github.com/tonym128/PSoXide-emulator](https://github.com/tonym128/PSoXide-emulator)**

Key enhancements included in this build:
1. **WebGL2 Fallback (`c7bad93`):** Downlevel limits and WebGL2 fallback on `wasm32` to avoid WebGPU incompatibilities across mobile and desktop browsers.
2. **Multi-Track CD-DA Audio (`5cffed8`):** Support for multi-track CUE sheets with real-time CD-DA audio playback for title and credits tracks.
3. **On-Screen Mobile Virtual Pad (`7112638`):** Host-to-guest button bitmask bridge for zero-latency touch controls.
4. **CD-ROM Sector Readiness Synchronization (`390fd19`):** Target sector prefetch readiness synchronization when seek/read commands are queued, ensuring stutter-free FMV streaming.

## Rebuilding the Web Player

To recompile the WebAssembly player bundle from source:

1. Clone or pull the emulator fork:
   ```bash
   git clone https://github.com/tonym128/PSoXide-emulator.git
   cd PSoXide-emulator
   ```
2. Build the slim web player distribution:
   ```bash
   python3 tools/build-web-player.py --out /tmp/player_build
   ```
3. Copy the generated files into this repository:
   ```bash
   rm -rf web/player/*
   cp -r /tmp/player_build/* /path/to/plattypus-psoxide/web/player/
   ```

## Local Testing

To test the Web Arcade locally with full Range request (HTTP 206) support:
```bash
make web
```
This serves http://localhost:8080.
