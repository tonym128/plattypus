# Plattypus: Tactical Espionage Action 🦆 (Sony PlayStation 1 / PSX)

[![CI](actions/workflows/ci.yml/badge.svg)](actions/workflows/ci.yml)

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
* **Hardware MDEC Cinematics & CD-XA Audio**: 320x240 full-screen video at 15 fps decoded via the PlayStation Motion Decoder (MDEC) coprocessor, with interleaved 37.8 kHz stereo **CD-XA ADPCM audio** streaming directly off disc sectors into the SPU CD audio mixer.
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
* **PlayStation Memory Card Integration**: 1-block save support with custom animated 16x16 3-frame BIOS save icon.
* **Clean Shared Architecture (`plattypus-core`)**: Pure `#![no_std]` game logic, save formats, and level geometry verified by 46 automated host test suites (`make test`).
* **Commercial Retail Packaging**: Production-ready print specifications for NTSC-U/C and PAL jewel cases, instruction manual, and silk-screened disc face (see [`packaging/PRESS_KIT.md`](packaging/PRESS_KIT.md) and [`packaging/`](packaging/)).

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
- `git` (the PSoXide SDK is a submodule and must be initialised)
- The Rust nightly pinned in [`rust-toolchain.toml`](rust-toolchain.toml).
  `rustup` installs it automatically on first build; it needs `rust-src` and
  `llvm-tools` for the `mipsel-sony-psx` cross build.
- Host C/C++ compiler and Python 3.

### Getting the source

The PSoXide SDK is a git submodule, so a plain clone is not enough.

```sh
git clone --recurse-submodules <repository-url>
cd plattypus-psoxide

# If you already cloned without --recurse-submodules:
git submodule update --init --recursive
```

The build fails with an unhelpful `psx-asset` manifest error if the `psoxide/`
submodule is missing, so check `ls psoxide/sdk` before building.

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

## 💿 Emulation & Real Hardware

### Recommended Emulators:
Open `dist/plattypus.cue` (after running `make disc`) in:
- **DuckStation** (Recommended — full hardware MDEC and SPU timing accuracy)
- **RetroArch** (Beetle PSX or SwanStation core)
- **PCSX-Redux** / **Mednafen**

### Real PlayStation Hardware:
Burn `dist/plattypus.cue` to a CD-R at low speed (4x) using `cdrdao` or ImgBurn, and boot on an original PlayStation console equipped with a modchip, UniROM, PSIO, or XStation ODE.
