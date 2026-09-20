# Plattypus: Tactical Espionage Action — Development Checklist

## Priority Phase 1: Recommended Immediate Steps (Foundations)

These 5 items form the core technical foundation required for a commercial-grade PlayStation experience.

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

- [ ] **3. DualShock Controller & Rumble Support**
  - [ ] Implement DualShock analog stick input protocol (smooth 360° walking/crawling analog speed control)
  - [ ] Implement dual-motor vibration feedback (small motor for heartbeats/alerts; heavy motor for explosions/hits)
  - [ ] Support controller unplug / replug detection without game freeze

- [ ] **4. VRAM Texturing & Gouraud Shading**
  - [ ] Create VRAM atlas layout (`0..1024x512`) with 4-bit / 8-bit color lookup tables (CLUT)
  - [ ] Paint low-res textures for Platty, sentries, crates, vehicles, and environmental tiles
  - [ ] Update renderer to use hardware textured quads (`gpu::draw_quad_textured`) with PS1 affine mapping
  - [ ] Implement hardware Gouraud directional vertex shading

- [ ] **5. Chapter 1 Tactical Boss Fight (Searchlight Mech)**
  - [ ] Create 3D multi-part model for the Sanctuary Searchlight Mech walker
  - [ ] Code boss behavior: dual sweeping searchlights, siren alert, shockwave stomp
  - [ ] Implement boss arena mechanics: crawling trenches, 3 destructible power conduits, and heat exhaust weak point
  - [ ] Add boss health bar HUD, victory cutscene, and transition to Chapter 2

---

## Phase 2: Tactical Platypus Abilities & Advanced Stealth Mechanics

- [ ] **Venomous Spur Strike (CQC)**
  - [ ] Add rear silent takedown trigger when sneaking up behind unalerted sentries
  - [ ] Create spur strike animation and star-daze stun effect
- [ ] **Electro-reception / Sonar Pulse**
  - [ ] Bind `TRIANGLE` hold to charging an electrical pulse wave
  - [ ] Render wall-penetrating outline blips for hidden sentries, yabbies, and vents
- [ ] **The Bill Box (Cardboard Disguise)**
  - [ ] Add collectible Cardboard Box item to inventory
  - [ ] Implement box concealment state: motionless = ignored by sentries; moving in vision = alert
- [ ] **Acoustic Surface Detection**
  - [ ] Tag floor tiles with acoustic materials: metal grating (loud), grass/water (quiet), pavement (medium)
  - [ ] Sentries turn to investigate nearby loud footstep sounds before raising an alarm

---

## Phase 3: Full 4-Chapter Campaign Expansion (12–16 Levels)

### Chapter 1: The Healesville Sanctuary
- [x] Level 1.1: Security Drainage Outflow (Current Stage 1 prototype)
- [ ] Level 1.2: Research Barracks & Laser Grid Maze
- [ ] Level 1.3: Perimeter Gate & Boss: Searchlight Mech

### Chapter 2: The Yarra River Wilds
- [x] Level 2.1: Upper Gorge 5-Lane River Rapids (Current Stage 2 prototype)
- [ ] Level 2.2: Dandenong Murky Mangroves & Cavern Maze
- [ ] Level 2.3: River Rapids Pursuit & Boss: Park Ranger Jet Ski

### Chapter 3: Melbourne Downtown
- [x] Level 3.1: Melbourne Neon Highway Frogger (Current Stage 3 prototype)
- [ ] Level 3.2: Flinders Street Laneways & Rooftop Catwalks
- [ ] Level 3.3: Antenna Tower & Boss: Sniper Kookaburra

### Chapter 4: Coastal Beachhead
- [x] Level 4.1: Coastal Dunes & Parasol 3D Platformer (Current Stage 4 prototype)
- [ ] Level 4.2: Pier Understructure & Deep Water Shark Trench
- [ ] Level 4.3: Burrow Defense & Final Boss: Dr. Cane Toad's Excavator

---

## Phase 4: Cinematics, Narrative & CODEC Polish

- [ ] **Cinematic Presentation**
  - [ ] Letterboxed widescreen presentation mode for story dialogue
  - [ ] Multi-camera cutscene scripting system (interpolated camera tracks, cuts, close-ups)
  - [ ] Encode opening cinematic and ending celebration into PS1 STR (MDEC) video format
- [ ] **CODEC Expansion**
  - [ ] Add multiple radio frequencies (e.g., Mom & Dad: 140.85, Wildlife Informant: 141.12, Save System: 140.96)
  - [ ] Add contextual radio dialogue for every stage, boss encounter, and puzzle

---

## Phase 5: Replayability, Modes & Commercial Polish

- [ ] **MGS Codename Mission Ranking**
  - [ ] Track stats: time elapsed, alert phases, rations eaten, guards neutralized, damage taken
  - [ ] Award end-game codenames (*Big Platypus*, *Tasmanian Devil*, *Lurking Echidna*, *Sly Possum*)
- [ ] **VR Training Simulator Mode**
  - [ ] Add VR Training menu option with 15 test stages (Sneaking, CQC, Runner, Platforming)
  - [ ] Time trial leaderboards saved to Memory Card
- [ ] **Unlockable Content & New Game+**
  - [ ] Tuxedo Platty costume (unlocked after beating game)
  - [ ] Stealth Camo Bandana (infinite air & invisible to radar)
  - [ ] Retro 1994 Flat-Shaded Wireframe mode

---

## Phase 6: Mastering, Packaging & Physical Publishing

- [ ] **Region & Standards Compliance**
  - [ ] Auto-detect 50Hz PAL / 60Hz NTSC with vertical display centering options
  - [ ] Multi-language text selector (English, French, German, Spanish, Japanese)
- [ ] **Disc Mastering**
  - [ ] Generate verified red-book CD-ROM master images (`.cue`/`.bin`) with CD-DA tracks
  - [ ] Optimize sector layout for maximum optical drive streaming performance and sub-2s loads
- [ ] **Physical Print & Publishing**
  - [ ] Front/back jewel case insert art (PAL blue border and NTSC-U jewel formats)
  - [ ] 20-page full-color instruction manual layout (story, bios, controls, notes)
  - [ ] Screen-printed disc surface art template
