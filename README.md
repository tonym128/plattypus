# Plattypus 🦆 (PSX / PlayStation 1)

A side-scrolling platformer for the original **Sony PlayStation (PS1 / PSX)**, developed in bare-metal **Rust** using the **[PSoXide](https://github.com/EBonura/PSoXide)** SDK.

---

## 📜 Story & Narrative

Platty is a contented platypus living at **Healesville Sanctuary** in Victoria, Australia. He chose sanctuary life for its pristine creek water, abundance of sweet freshwater yabbies, and peaceful eucalyptus groves.

One evening, a kookaburra courier delivers an urgent letter from his parents in his satchel:
> *"Dearest Platty, We have wonderful news! You are going to be a big brother! An egg has hatched in the coastal burrow. Please hurry home as fast as you can to be there for the family! Love, Mom & Dad"*

Determined to get home to meet his new baby sibling, Platty packs his satchel, keeping the letter close, and embarks on an epic journey across Victoria.

---

## 🗺️ The 4 Acts

1. **Act 1: Night Escape from Healesville Sanctuary**
   - Sneak past sleepy sanctuary keepers and avoid their flashlight beams.
   - Leap across wooden boardwalks, pond reeds, and security wire fences.
   - Slide down the park’s overflow water flume to escape into the wild!
2. **Act 2: The Bushland Backroads & Billabongs**
   - Deep eucalyptus bushland, gravel roads, and winding creeks of the Yarra Valley.
   - High-speed mud and grass belly-slides down steep red-earth gullies.
   - Dodge nocturnal wombats charging out of burrows and swim across fast-flowing billabongs.
3. **Act 3: Melbourne City Transit & Rooftops**
   - Navigate the urban neon jungle, fire escapes, brick chimneys, and telephone wires.
   - Sprint across the roofs of moving city trams.
   - Dive through underground stormwater drainage tunnels leading toward the bay.
4. **Act 4: The Coastal Native Shore & Estuary**
   - Golden beaches, coastal cliffs, rock pools, and crashing ocean waves.
   - Swim through the tidal estuary against the ocean surge.
   - Arrive at the family burrow for the heartwarming reunion with Mom, Dad, and the newly hatched baby sister Pip!

---

## 🎮 Controls

| Button | Action | Description |
| :--- | :--- | :--- |
| **D-Pad / Left Stick** | Move / Swim | Walk and run on land (waddle); 360° directional swimming in water. |
| **Cross ($\times$)** | Jump / Flutter / Paddle | Jump on land; press while falling for a flutter kick; paddle faster in water. |
| **Down + Cross** | **Belly Slide** | Dive into a low-profile belly slide! Accelerates down slopes and glides under barriers. |
| **Square ($\Box$)** | **Tail Thwap** | Spin 180° to strike with Platty’s beaver tail, stunning enemies and breaking obstacles. |
| **Triangle ($\triangle$)** | **Bill Sense** | Platypus electro-reception! Emits a pulsing radar wave revealing hidden yabbies and paths. |
| **Start** | Start / Advance | Start game, progress dialogue, and continue between acts. |

---

## 🛠️ Building & Mastering

### Prerequisites
- Rust nightly toolchain with `rust-src` and `llvm-tools` (automatically managed by rustup).
- Host C/C++ compiler and Python 3.

### Build Targets

```sh
# Build both the MIPS binary and master the bootable PS1 disc
make disc

# Output files generated in dist/:
#   dist/plattypus.exe  - MIPS R3000 bare-metal executable
#   dist/plattypus.bin  - Raw 2352-byte/sector disc image
#   dist/plattypus.cue  - Disc cue sheet
```

---

## 🕹️ Playing

### Emulators
Open [`dist/plattypus.cue`](dist/plattypus.cue) in any PlayStation emulator:
- **DuckStation** (Recommended)
- **RetroArch** (Beetle PSX or SwanStation core): `make run`
- **PCSX-Redux** / **Mednafen**

### Real PlayStation Hardware
Burn `dist/plattypus.cue` to a CD-R using your favorite burner (e.g. `cdrdao` or ImgBurn) at low speed (4x), and boot on an original PlayStation with a modchip, PSIO, or UniROM.

---

## ⚙️ Technical Architecture

- **Platform**: Sony PlayStation 1 (MIPS R3000A @ 33.868 MHz).
- **Video**: 320 $\times$ 240 NTSC, double-buffered framebuffers in 1MB VRAM, VBlank IRQ synchronization (`wait_vblank()`).
- **Math**: Deterministic 16.16 fixed-point arithmetic (`Fixed`) for smooth 60fps physics without floating-point emulation.
- **Audio**: Native SPU sound processor with ADPCM samples uploaded to SPU RAM (`jump`, `pickup_coin`, `swoosh`, `hit_punch`, `ui_beep`, `footstep`, `ui_select`).
- **Rendering**: Hardware GPU commands (flat quads, triangles, blended translucent lighting cones, line primitives, and bitmap fonts).
