# PLATTYPUS: TACTICAL ESPIONAGE ACTION
## Official Press Kit & Retail Fact Sheet
### Bare-Metal Rust on 1994 Hardware (Sony PlayStation 1 / PSX)

---

## 📌 Fact Sheet

* **Title**: Plattypus: Tactical Espionage Action
* **Developer**: Burrow Command Studios
* **Platform**: Sony PlayStation 1 (PS1 / PSX)
* **Release Format**: CD-ROM (CUE/BIN Image), CD-R Homebrew, Physical Retail Jewel Case Edition
* **Target Audience**: Retro gaming enthusiasts, PS1 collectors, Metal Gear Solid fans, Rust embedded developers
* **Language Support**: 5 Languages (English, Français, Deutsch, Español, 日本語 / Romaji)
* **Hardware Compatibility**: All PS1 consoles (NTSC-U/C, PAL, NTSC-J) via Modchip, UniROM, XStation, PSIO; Emulators (DuckStation, RetroArch, PCSX-Redux, Mednafen)
* **Peripherals Supported**: Standard Digital Controller, DualShock® Analog Controller (360° analog stick + dual-motor vibration feedback), PlayStation Memory Card (1 Block)
* **Serial Number**: `BASLUS-00001` (Unified Master ID) / `SCES-00001` (PAL Catalog Reference)
* **License**: GPL-2.0-or-later

---

## 🎯 Elevator Pitch

> **Metal Gear Solid meets Australian Wildlife — built entirely from scratch in bare-metal Rust for 1994 Sony PlayStation hardware.**

Step into the webbed feet of **Agent Platty**, an elite bio-augmented duck-billed operative on an urgent stealth mission: infiltrate a heavily guarded nature reserve, bypass corporate searchlight mechs and patrol boats, and reach Burrow Command before the birth of his baby sister Pip. 

Featuring genuine 3D environments accelerated by the PS1 Geometry Transformation Engine (GTE), full-motion MDEC video cutscenes with interleaved 37.8 kHz CD-XA ADPCM stereo audio, authentic multi-frequency CODEC radio transmissions, cardboard box subterfuge, and dual-motor DualShock® rumble.

---

## 🌟 Key Selling Points & Features

### 1. Genuine Bare-Metal Rust Engineering
* Compiled directly to **MIPS R3000A** bare-metal machine code using `rustc` and the open-source **PSoXide** SDK.
* Zero runtime overhead, `#![no_std]` architecture, 100% manual memory management adhering strictly to the PS1's 2 MB main RAM constraint.
* Clean separation of concerns with an independent shared logic crate (`plattypus-core`) validated by 46 automated host test suites.

### 2. Full-Motion Video with Hardware MDEC & CD-XA Audio
* High-resolution 320x240 full-screen cinematics at 15 fps.
* Decoded using the PlayStation’s specialized **Motion Decoder (MDEC)** coprocessor.
* **Interleaved CD-XA Form 2 ADPCM Audio**: 37.8 kHz stereo audio streamed directly off disc sectors straight into the SPU CD audio mixer, offloading the CPU entirely.

### 3. Tactical Stealth Infiltration
* **Soliton Radar System**: Real-time overhead radar display tracking sentry positions, directional headings, and patrol cones.
* **Close Quarters Combat (CQC)**: Neutralize patrolling guards from behind with venomous calcaneus ankle spurs.
* **The Bill Box**: Hide inside Melbourne Fruit Co. cardboard crates to evade roving searchlights and sentries.
* **Dual-Stance Movement**: Transition seamlessly between upright running and prone belly-crawls to slide beneath laser tripwires and into air ventilation ducts.
* **Aquatic Submersion**: Dive underwater in river rapids and mangrove channels to swim beneath surface patrol cutters.

### 4. Rich 4-Chapter Campaign + VR Simulator
* **Chapter 1 — Healesville Sanctuary**: Drainage Outflow canal, Research Barracks stealth maze, and the Searchlight Mech boss battle.
* **Chapter 2 — Yarra River Wilds**: 5-lane white-water gorge surfing, Dandenong Mangroves sonar maze, and high-speed Ranger Jet Ski pursuit.
* **Chapter 3 — Melbourne Downtown**: Rush-hour highway crossing, Flinders Laneways urban rooftops, and Antenna Tower sniper duel against Kooky the Kookaburra.
* **Chapter 4 — Coastal Beachhead**: Wind-swept dunes platforming, Pier Shark Trench underwater infiltration, and Dr. Cane Toad's Amphibian Excavator climax.
* **VR Training Simulator**: 4 discrete VR training missions (Sneaking, CQC Spurs, Sonar Labyrinth, and Speed Sprint).

### 5. Classic 90s PlayStation Polish
* **Multi-Frequency CODEC Radio**: Authentic wireless communications with frequency tuning (Burrow Command: 140.85, Field Save: 140.96, Bushland Intel: 141.12, Tactical Gear: 141.80).
* **Memory Card BIOS Integration**: 1-block save file featuring a custom 16x16 3-frame animated icon visible in the official PlayStation BIOS Memory Card manager.
* **Red Book CD-DA Title Soundtrack**: Uncompressed 44.1 kHz 16-bit stereo CD audio track mastered onto Track 2.
* **DualShock® Analog & Rumble Support**: Full 360° analog sneaking sensitivity and dual-motor force feedback during explosions, sonar discharges, and hits.

---

## 📖 Story Synopsis

Deep in the pristine bushlands of Victoria, Australia, corporate industrial encroachers led by the ruthless Dr. Cane Toad have quarantined the Healesville wildlife sanctuary. Heavy surveillance walkers, acoustic tripwires, and armed river patrol craft lock down every waterway.

Just as night falls, an urgent high-priority transmission crackles over Agent Platty's encrypted CODEC frequency: Burrow Command reports that a golden egg has been laid in the coastal nursery burrow—baby sister Pip is hatching! 

With zero time to spare, Platty dons his combat headband, checks his venom spurs, and embarks on a four-chapter tactical extraction across Victoria's wild waterways and neon urban centers.

---

## 👥 Characters & Cast

* **Agent Platty**: The protagonist. An amphibious, venom-spurred covert operative equipped with electro-receptive bill sensors and high-altitude combat headband.
* **Commander Platty (Dad)**: Burrow Command tactical director (CODEC 140.85). Provides strategic counsel and mission objectives.
* **Dr. Platty (Mom / Scribe)**: Chief medical officer and mission chronicler (CODEC 140.96). Records mission progress to the memory card.
* **Wally the Wombat**: Burrow engineering and ordinance specialist (CODEC 141.80). Supplies tactical gear advice and cardboard box tips.
* **Pip**: Platty's newly-hatched baby sister waiting at Coastal Nursery Burrow.
* **Dr. Cane Toad**: Syndicate commander. Deploys heavy industrial excavation mechs to clear wetlands for corporate expansion.
* **Sniper Kooky**: Expert avian sharpshooter perched atop the city broadcast antenna tower.

---

## 📦 Retail & Packaging Specs

Burrow Command Studios has prepared complete commercial-grade physical packaging specifications:
* **NTSC-U/C Single Jewel Case**: Standard 120mm front booklet with staple binding, back tray card with spine titles, technical compatibility badges, and ESRB 10+ rating metadata.
* **PAL Thick Double Jewel Case**: European double-thickness case with signature left blue banner, multi-language back blurbs in 5 languages, and PEGI/ELSPA rating icons.
* **Screen-Printed CD-ROM Surface**: 3-color silk-screen disc face artwork adhering strictly to ECMA-130 Red Book CD tolerances.

---

## 📸 Media Assets

* **Hero Screenshot**: `Images/frontscreen.png` (320x240 native PS1 frame buffer capture of Agent Platty ready for deployment).
* **Vector Jewel Case Covers**:
  * `packaging/jewel_case_cover_ntsc.svg`
  * `packaging/jewel_case_back_ntsc.svg`
  * `packaging/jewel_case_pal_double.svg`
  * `packaging/disc_face_art.svg`
* **Instruction Manual**: Complete 20-page retro manual in `packaging/MANUAL.md`.

---

## 🛠️ Build & Run Quickstart

```bash
# Clone with submodules
git clone --recurse-submodules https://github.com/EBonura/plattypus-psoxide.git
cd plattypus-psoxide

# Run game logic regression test suite (46 test suites)
make test

# Compile MIPS R3000 bare-metal PSX binary
make exe

# Master complete bootable PS1 CD-ROM disc image (with CD-DA audio & XA video)
make disc
```

---
*For press inquiries, review builds, or packaging inquiries, contact Burrow Command Studios.*
