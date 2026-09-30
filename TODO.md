# Plattypus: Tactical Espionage Action — Development Checklist

> **Status: complete — kept as a record of what was built.**
> Every checkbox below is ticked, including the two items filed under "Active
> Issues" (inverted controls, and stages not resetting after death). Both were
> fixed. This file is a historical checklist, not an open work queue; current
> status and remaining work are tracked in [REVIEW.md](REVIEW.md).

## Priority Phase 1: Recommended Immediate Steps (Foundations)

These 5 items form the core technical foundation required for a commercial-grade PlayStation experience.

## 🚨 Active Issues & Bug Fixes

- [x] **Fix Inverted Game Controls (Up and Down Flipped)**
  - [x] Invert vertical movement controls for D-Pad (`btn_up` and `btn_down` in `game/src/platypus.rs` so UP moves North/-Z and DOWN moves South/+Z)
  - [x] Invert DualShock analog stick Y-axis (`sy`) and update angle calculations so stick up moves forward/north
  - [x] Verify attract mode simulated inputs and demo playback reflect the corrected axis mapping
- [x] **Stages aren't reset after death and restart, the player dies over and over, please fix**
  - [x] Guard `take_damage` against repeating damage while health is 0
  - [x] Add debounce delay to GameOver screen (45 frames) to prevent accidental immediate restart
  - [x] In `load_act`: snap camera immediately to player spawn point to eliminate disorienting camera drift
  - [x] Grant 90 frames (1.5 seconds) of respawn invulnerability with flashing sprite
  - [x] Enable jumping in Act 2 River Rapids so Platty can leap over logs, snakes, and spiders, and duck under low branches
---

- [x] **1. SPU / CD-DA Audio Engine**
  - [x] Implement SPU ADPCM sound effects driver (24 hardware voices)
  - [x] Add sound effects: footsteps (metal, grass, pavement), sentry alert exclamation (`!`), dive splash, spur strike
  - [x] Implement CD-DA or XA audio streaming for dynamic background music
  - [x] Compose/integrate ambient stealth BGM and high-intensity alert BGM
  - [x] Implement CODEC chime audio and digitized voice transmission clips for Burrow Command

- [x] **2. Memory Card System (1 Block)**
  - [x] Implement PS1 BIOS memory card read/write routines
  - [x] Define save game data struct (act progression, high scores, codename records, settings)
  - [x] Design and embed 16x16 3-frame animated icon for the PlayStation BIOS Memory Card manager
  - [x] Add save checkpoint triggers after each completed act and boss encounter

- [x] **3. DualShock Controller & Rumble Support**
  - [x] Implement DualShock analog stick input protocol (smooth 360° walking/crawling analog speed control)
  - [x] Implement dual-motor vibration feedback (small motor for heartbeats/alerts; heavy motor for explosions/hits)
  - [x] Support controller unplug / replug detection without game freeze

- [x] **4. VRAM Texturing & Gouraud Shading**
  - [x] Create VRAM atlas layout (`0..1024x512`) with 4-bit / 8-bit color lookup tables (CLUT)
  - [x] Paint low-res textures for Platty, sentries, crates, vehicles, and environmental tiles
  - [x] Update renderer to use hardware textured quads (`gpu::draw_quad_textured`) with PS1 affine mapping
  - [x] Implement hardware Gouraud directional vertex shading

- [x] **5. Chapter 1 Tactical Boss Fight (Searchlight Mech)**
  - [x] Create 3D multi-part model for the Sanctuary Searchlight Mech walker
  - [x] Code boss behavior: dual sweeping searchlights, siren alert, shockwave stomp
  - [x] Implement boss arena mechanics: crawling trenches, 3 destructible power conduits, and heat exhaust weak point
  - [x] Add boss health bar HUD, victory cutscene, and transition to Chapter 2

---

## Phase 2: Tactical Platypus Abilities & Advanced Stealth Mechanics

- [x] **Venomous Spur Strike (CQC)**
  - [x] Add rear silent takedown trigger when sneaking up behind unalerted sentries
  - [x] Create spur strike animation and star-daze stun effect
- [x] **Electro-reception / Sonar Pulse**
  - [x] Bind `TRIANGLE` hold to charging an electrical pulse wave
  - [x] Render wall-penetrating outline blips for hidden sentries, yabbies, and vents
- [x] **The Bill Box (Cardboard Disguise)**
  - [x] Add collectible Cardboard Box item to inventory
  - [x] Implement box concealment state: motionless = ignored by sentries; moving in vision = alert
- [x] **Acoustic Surface Detection**
  - [x] Tag floor tiles with acoustic materials: metal grating (loud), grass/water (quiet), pavement (medium)
  - [x] Sentries turn to investigate nearby loud footstep sounds before raising an alarm

---

## Phase 3: Full 4-Chapter Campaign Expansion (12–16 Levels)

### Chapter 1: The Healesville Sanctuary
- [x] Level 1.1: Security Drainage Outflow (Current Stage 1 prototype)
- [x] Level 1.2: Research Barracks & Laser Grid Maze
- [x] Level 1.3: Perimeter Gate & Boss: Searchlight Mech

### Chapter 2: The Yarra River Wilds
- [x] Level 2.1: Upper Gorge 5-Lane River Rapids (Current Stage 2 prototype)
- [x] Level 2.2: Dandenong Murky Mangroves & Cavern Maze
- [x] Level 2.3: River Rapids Pursuit & Boss: Park Ranger Jet Ski

### Chapter 3: Melbourne Downtown
- [x] Level 3.1: Melbourne Neon Highway Frogger (Current Stage 3 prototype)
- [x] Level 3.2: Flinders Street Laneways & Rooftop Catwalks
- [x] Level 3.3: Antenna Tower & Boss: Sniper Kookaburra

### Chapter 4: Coastal Beachhead
- [x] Level 4.1: Coastal Dunes & Parasol 3D Platformer (Current Stage 4 prototype)
- [x] Level 4.2: Pier Understructure & Deep Water Shark Trench
- [x] Level 4.3: Burrow Defense & Final Boss: Dr. Cane Toad's Excavator

---

## Phase 4: Cinematics, Narrative & CODEC Polish

- [x] **Cinematic Presentation**
  - [x] Letterboxed widescreen presentation mode for story dialogue
  - [x] Multi-camera cutscene scripting system (interpolated camera tracks, cuts, close-ups)
  - [x] Encode opening cinematic and ending celebration into PS1 STR (MDEC) / VID format with hardware SPU audio and CD streaming
- [x] **CODEC Expansion**
  - [x] Add multiple radio frequencies (e.g., Mom & Dad: 140.85, Wildlife Informant: 141.12, Save System: 140.96)
  - [x] Add contextual radio dialogue for every stage, boss encounter, and puzzle

---

## Phase 5: Replayability, Modes & Commercial Polish

- [x] **MGS Codename Mission Ranking**
  - [x] Track stats: time elapsed, alert phases, rations eaten, guards neutralized, damage taken
  - [x] Award end-game codenames (*Big Platypus*, *Tasmanian Devil*, *Lurking Echidna*, *Sly Possum*, *Duckbill Rookie*)
- [x] **VR Training Simulator Mode**
  - [x] Add VR Training menu option with standalone test stages (VR-01 Sneaking, VR-02 CQC, VR-03 Sonar, VR-04 Speed Hurdles)
  - [x] Progression flags & high scores saved to Memory Card
- [x] **Unlockable Content & New Game+**
  - [x] Tuxedo Platty costume (unlocked after beating game)
  - [x] Stealth Camo Bandana (optical shimmering active camouflage)
  - [x] Retro 1994 Flat-Shaded Wireframe mode

---

## Phase 6: Mastering, Packaging & Physical Publishing

- [x] **Region & Standards Compliance**
  - [x] Auto-detect 50Hz PAL / 60Hz NTSC with vertical display centering options
  - [x] Multi-language text selector (English, French, German, Spanish, Japanese)
- [x] **Disc Mastering**
  - [x] Generate verified red-book CD-ROM master images (`.cue`/`.bin`) with CD-DA tracks
  - [x] Optimize sector layout for maximum optical drive streaming performance and sub-2s loads
- [x] **Physical Print & Publishing**
  - [x] Front/back jewel case insert art (PAL blue border and NTSC-U jewel formats)
  - [x] 20-page full-color instruction manual layout (story, bios, controls, notes)
  - [x] Screen-printed disc surface art template
