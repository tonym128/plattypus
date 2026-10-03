# Plattypus: Tactical Espionage Action 🦆 (Sony PlayStation 1 / PSX)

[![CI](https://github.com/tonym128/plattypus/actions/workflows/ci.yml/badge.svg)](https://github.com/tonym128/plattypus/actions/workflows/ci.yml)

A 3D tactical stealth action infiltration thriller for the original **Sony PlayStation (PS1 / PSX)**, developed in bare-metal **Rust** using the **[PSoXide](https://github.com/EBonura/PSoXide)** SDK.

---

## 📜 Mission Dossier & Story

Deep in Victoria, Australia, peaceful wildlife habitats are under threat. Healesville Sanctuary has been locked down by corporate security walkers, surveillance drones, and laser fences. 

One evening, an encrypted transmission reaches Agent Platty's tactical earpiece from Burrow Command (Mom & Dad):
> *"Platty, darling! A golden egg was laid in Coastal Burrow! You're going to be a big brother to baby sister Pip! But the reserve is on high alert — searchlight mechs and patrol craft block every exit. Execute tactical espionage action to escape and hurry home!"*

Equipped with his combat headband, venomous ankle spurs, and bio-electric electro-sonar bill, Platty must infiltrate 4 high-security chapters, disable lethal combat mechs, and reach the coastal nursery burrow before Dr. Cane Toad's excavator strikes!

---

## 🎮 Key Features

* **Bare-Metal Rust on 1994 Hardware**: Compiled to native MIPS R3000A machine code using `rustc` with `#![no_std]`, targeting 2 MB of main RAM with zero operating system dependencies.
* **3D Tactical Espionage Gameplay**: Fully 3D environments with GTE-accelerated fixed-point math, directional Gouraud shading, and affine texture mapping.
* **Hardware MDEC Cinematics**: 320x240 full-screen video at 15 fps decoded on the PlayStation Motion Decoder (MDEC) coprocessor and streamed off disc sectors into VRAM over DMA channel 2. Cinema audio currently plays from embedded SPU samples; the CD-XA path is built but not yet enabled in the shipped player ([why](docs/perf/why-6fps.md)).
* **Red Book CD-DA Title Soundtrack**: Uncompressed 44.1 kHz 16-bit stereo CD digital audio mastered directly onto Track 2.
* **Soliton Radar System**: Real-time tactical radar displaying enemy positions, patrol headings, and vision cones.
* **Multi-Frequency CODEC Radio**: Authentic MGS-style wireless communications with frequency tuning (Burrow Command: 140.85, Field Save: 140.96, Bushland Intel: 141.12, Tactical Gear: 141.80).
* **Tactical Infiltration Abilities**:
  * **Venom Spur Strike (CQC)**: Execute silent rear takedowns on unaware guards.
  * **Electro-Sonar Detection**: Hold to charge and discharge a bio-electric wave revealing hidden yabbies and stunning nearby electronics.
  * **The Bill Box**: Conceal yourself under a cardboard shipping crate to evade roving sentries.
  * **Belly Crawl / Prone Mode**: Crawl through low ventilation shafts and beneath laser tripwires.
  * **Subsurface Diving**: Submerge underwater to swim beneath surface searchlights and patrol craft.
* **4 Epic Chapters (12 Campaign Acts + 4 Boss Encounters)**:
  * **Chapter 1**: Healesville Sanctuary (Drainage Outflow, Barracks Maze, Searchlight Mech Boss).
  * **Chapter 2**: Yarra River Wilds (5-Lane Gorge Rapids, Mangrove Sonar Caverns, Ranger Jet Ski Pursuit Boss).
  * **Chapter 3**: Melbourne Downtown (Rush-Hour Highway Frogger, Flinders Laneways, Sniper Kookaburra Boss).
  * **Chapter 4**: Coastal Beachhead (Sandstone Cliff Platforming, Pier Shark Trench, Dr. Cane Toad's Excavator Climax).
* **VR Training Simulator**: 4 standalone training missions (Sneaking, CQC, Sonar Labyrinth, Speed Hurdles).
* **DualShock® Analog & Rumble Support**: Full 360° analog stealth speed control and dual-motor vibration feedback.
* **PlayStation Memory Card Integration**: 1-block save with a custom 16x16 16-colour BIOS save icon.
* **Clean Shared Architecture (`plattypus-core`)**: Pure `#![no_std]` game logic, save formats, and the level generators themselves -- so the host suite tests the real code, not a copy of it. 23 automated host test suites (`make test`), including an exhaustive checksum/bit-flip sweep and a full 4-D sweep of the rank evaluator.
* **Packaging Specifications**: Print specifications for NTSC-U/C and PAL jewel cases, instruction manual, and silk-screened disc face (see [`packaging/PRESS_KIT.md`](packaging/PRESS_KIT.md) and [`packaging/`](packaging/)). The artwork deliberately carries **no rating badge, no Sony Seal of Quality and no barcode** -- none has been issued or licensed. Nothing is for sale; see [`LICENSE`](LICENSE).

---

## 🕹️ Controls

| Button | Tactical Action (Infiltration) | Water / Submerged | CODEC / Tuner Mode |
| :--- | :--- | :--- | :--- |
| **D-Pad / Left Stick** | 360° Movement / Stealth Stalk | Swim 360° | Tune Radio Frequency |
| **Cross ($\times$)** | Jump / Flutter | Submerge / Dive Deep | Advance Text / Confirm |
| **Circle ($\bigcirc$)** | Toggle Belly Crawl (Prone) | Paddle Forward | Cancel / Back |
| **Square ($\Box$)** | Venom Spur Strike (CQC Takedown) | Submerge / Dive Deep | - |
| **Triangle ($\triangle$)** | Charge / Fire Electro-Sonar Pulse | Charge Sonar | - |
| **L1** | Toggle Cardboard Box Disguise | - | - |
| **Select** | Open CODEC Wireless Radio | Open CODEC Radio | Exit CODEC |
| **Start** | In-Game Pause Menu | In-Game Pause Menu | Skip Transmission |

---

## 🛠️ Building & Mastering

### Prerequisites
- `git` (the PSoXide SDK is vendored in-tree under `psoxide/`; no submodule initialisation is needed)
- The Rust nightly pinned in [`rust-toolchain.toml`](rust-toolchain.toml).
  `rustup` installs it automatically on first build; it needs `rust-src` and
  `llvm-tools` for the `mipsel-sony-psx` cross build.
- Host C/C++ compiler and Python 3.

### Getting the source

The PSoXide SDK is vendored in this repository, so a plain clone is enough --
there is no submodule to initialise.

```sh
git clone https://github.com/tonym128/plattypus.git
cd plattypus
```

Check `ls psoxide/sdk` before building; the cross build needs it present.

### Build Targets

```sh
# Run automated host-side game logic tests (saves, codenames, acts, collisions)
make test

# Compile MIPS R3000 bare-metal executable
make exe

# Master complete bootable PS1 disc image (CD-DA audio + MDEC video streams)
make disc
```

### Output Files (`dist/`)

These are build products and are **not** checked in. Run `make disc` to produce
them; the directory is listed in `.gitignore`.

- `dist/plattypus.exe` — Bare-metal MIPS R3000 PlayStation executable
- `dist/plattypus.bin` — Raw 2352-byte/sector disc image
- `dist/plattypus.cue` — Red-book disc cue sheet with CD-DA title audio track

---

## 📚 Documentation

| Doc | What it is |
|---|---|
| [`docs/perf/why-6fps.md`](docs/perf/why-6fps.md) | How the intro cinematic went from 6.00 to 15.00 fps: the measurements, the fixes that did *not* work, and why the codec mattered more than the scheduler |
| [`packaging/MANUAL.md`](packaging/MANUAL.md) | Full instruction manual |
| [`packaging/PRESS_KIT.md`](packaging/PRESS_KIT.md) | Fact sheet, synopsis, character list, media assets |
| [`packaging/JEWEL_CASE_SPEC.md`](packaging/JEWEL_CASE_SPEC.md) · [`packaging/DISC_SURFACE_SPEC.md`](packaging/DISC_SURFACE_SPEC.md) | Print specifications |
| [`ASSETS.md`](ASSETS.md) | Every third-party asset, its origin and its licence |
| [`CHANGELOG.md`](CHANGELOG.md) | Release notes |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to build, test and contribute |
| [`PLAN.md`](PLAN.md) · [`TODO.md`](TODO.md) | Historical plan and build checklist (superseded -- kept for context) |
| [`LICENSE`](LICENSE) | GPL-2.0-or-later |

---

## 💿 Emulation & Real Hardware

### Recommended Emulators:
Open `dist/plattypus.cue` (after running `make disc`) in:
- **DuckStation** (Recommended — full hardware MDEC and SPU timing accuracy)
- **RetroArch** (Beetle PSX or SwanStation core)
- **PCSX-Redux** / **Mednafen**

### Real PlayStation Hardware:
Burn `dist/plattypus.cue` to a CD-R at low speed (4x) using `cdrdao` or ImgBurn, and boot on an original PlayStation console equipped with a modchip, UniROM, PSIO, or XStation ODE.
