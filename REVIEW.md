# Plattypus: Tactical Espionage Action — Multi-Perspective Project Review

**Review date:** 2026-09-30
**Commit reviewed:** `832a502` (working tree clean except untracked `.opencode/`)
**Scope:** `game/src/*` (11,147 LOC, 14 files), `tools/test_game_logic`, `Makefile`, `packaging/`, root docs, repo hygiene.

### What this document is

Five independent reviews of the same tree — Principal Developer, Senior Engineer, Product Designer, Marketing, Testing — with a merged priority plan at the end. Every finding carries a `file:line` reference and was verified against the current source. Where a previous revision of this file claimed a fix, the claim is re-tested here.

### Prior review status (re-verified)

The previous revision of this file ended with "All high, medium, and low priority feedback items have been systematically resolved." **That claim was inaccurate.** Re-verified against the current tree:

| Prior item | Claimed | Actually |
|---|---|---|
| PD-2 (embedded assets) | Resolved | **Still open** — 782 KB of `include_bytes!`, 74% of the executable |
| PD-6 (test struct duplication) | (absent from table) | **Still open** — harness has zero dependencies |
| QA-6 (memory card full) | (absent from table) | **Still open** — no `Full` handling in `save.rs` |
| UX-7 (unreachable content) | (absent from table) | **Worse than described** — see UX-1 |
| PD-5 (`fixed.rs` removed) | Resolved | Correct, but the finding text still describes the file |
| PD-7 (constant extraction) | Resolved | Partially — 3 constants added, ~60 balance literals remain |

The prior review also cited stale line counts (`game.rs` 886 → now 1,093; `renderer.rs` 2,377 → now 2,688) and described a 4-chapter game as "16 playable stages" without reconciling the two. Findings below supersede it.

---

## 📊 Snapshot

| Metric | Value |
|---|---|
| Game source | 11,147 LOC / 14 files, `no_std` bare-metal Rust |
| Largest module | `renderer.rs` — 2,688 lines, 38 distinct concerns |
| Built executable | 1,052,672 bytes (`.text + .data` = 1,050,624) |
| Embedded static assets | 782,000 bytes (74.4% of the binary) |
| RAM budget | ~1,147,000 of ~1,984,000 usable bytes (58%) |
| VRAM declared | ~467,712 of 1,048,576 bytes (45%) — no active overlap |
| Worst-case draw load | ~32,900 quads/frame (realistic single act: ~2,100) |
| Test coverage | 11 suites, 63 assertions, 0 `#[test]`, 0 dependencies on game code |
| CI | **None.** The only workflow is inside the `psoxide` submodule |
| Tracked repo size | 66.6 MB, of which ~28.6 MB is unreferenced `.mp3`/`.mp4` |

---

# 🏗️ 1. Principal Developer — Architecture & Technical Debt

*Focus: module boundaries, state model, data ownership, maintainability.*

The game is 11k lines of clean, idiomatic `no_std` Rust with a defensible layering (`main` → `game` → `renderer`/`entities`/`platypus`/`level`/`save`). The problems are not in the layering — they are in **state ownership**: three subsystems each hold a private, divergent copy of "how far the player has got", and nothing validates loaded data.

### PD-1 — Three divergent implementations of "commit progress" 🔴

Progress is written to the save at three sites, with different semantics:

| Site | `unlocked_act` | `total_yabbies` |
|---|---|---|
| `game.rs:785` (stage clear) | `next.max(cur).min(11)` ✅ clamped | **`saturating_add`** ⚠️ |
| `game.rs:866` (CODEC scribe) | `act.max(cur)` 🔴 **no clamp** | `max` |
| `game.rs:887` (StageClear advance) | `next.min(11).max(cur)` ✅ | `max` |

This is the root cause of QA-1 (below). Three copies of one invariant will keep drifting. Collapse into a single method:

```rust
impl Game {
    fn commit_progress(&mut self, next: Option<Act>) { /* the one place unlocked_act is written */ }
}
```

### PD-2 — `unlocked_act` is overloaded for two meanings 🟠

`game.rs:187` treats `unlocked_act >= 11` as "campaign complete" (shows STAGE SELECT), but `game.rs:780-785` sets `unlocked_act = 11` merely to mean "4-3 is now available". The `else` branch at `game.rs:195-200` — the one that would load Act 4-3 from the title screen — is therefore **unreachable whenever 4-3 is the pending mission**. The final boss is reachable only by playing linearly or via Stage Select.

Use a separate `campaign_completed: bool`, or a sentinel `12` meaning "all clear".

### PD-3 — "NEW CAMPAIGN" never resets the save 🟠

`game.rs:203-232` resets `mission_stats` and calls `platty.reset_for_new_game()`, but **never touches `self.save_data`**. There is no `save_data = SaveData::new()` anywhere in the crate. Because every writer uses `.max()`, a save with `unlocked_act == 11` is permanently sticky: after "NEW CAMPAIGN" the title still offers STAGE SELECT, and the linear campaign can never be replayed from 1-1.

### PD-4 — `renderer.rs` is a 2,688-line monolith with 38 concerns 🟠

The module interleaves GPU primitives, level-tile presentation, actor models, HUD, seven front-end screens, and five inline i18n string tables. Suggested decomposition, with the boundaries that already exist in the file:

| Extract | Lines | Rationale |
|---|---|---|
| `prim.rs` | 617–872 | GTE projection + primitive emit. Pure, stateless, no game knowledge. |
| `traversal.rs` | 134–256 | Depth sort; 11 near-identical entity loops |
| `tiles.rs` | 398–615 | Tile dispatcher + shaders |
| `models/` | 1003–1654 | Actor models; the costume palette at 1352–1383 is a data table masquerading as a `match` |
| `hud.rs` | 1709–1925 | In-game HUD |
| `screens.rs` + `lang.rs` | 2025–2687 | Seven screens + ~60 lines of inline string data |

A related smell: `Renderer` (`renderer.rs:45-59`) holds GPU output state, camera transform, **and user preferences** (`costume`, `wireframe`, `language`, `video_mode`, `screen_offset_*`). Preferences are loaded from `SaveData` in `game.rs:81-85` and mirrored in, making the renderer a second source of truth for save data.

### PD-5 — Gameplay/level layout logic has leaked into the renderer 🔴

```rust
// renderer.rs:415
if gz == 18 || gz == 12 { /* road markings */ }
// renderer.rs:419
if gx == 4 || gx == 19 { /* sidewalk   */ }
// renderer.rs:425
if gx == 6  || gx == 22 { /* dunes     */ }
```

The renderer is deciding which *cells are roads*. This is `Level`'s job. The consequences are already visible: the act-1-2 laser bug below (SE/LX-1) is a symptom of exactly this class of ordering hazard, and `draw_stage_select_menu` (`renderer.rs:2621`) discards its `_unlocked_act` argument because it has no access to progression state.

### PD-6 — Save data is serialized as a raw struct blob including padding 🔴

```rust
// save.rs:355-360
let payload = unsafe {
    core::slice::from_raw_parts(data as *const SaveData as *const u8, core::mem::size_of::<SaveData>())
};
```

`SaveData` is `#[repr(C)]`, not packed. Layout: `magic[4]` + `version` + `unlocked_act` = 6 bytes, then `highest_score: u32` forces 4-byte alignment → **bytes 6–7 are padding**. Writing the whole struct therefore (a) persists 2 bytes of uninitialized memory, (b) leaks stack/heap residue, and (c) makes the on-card bytes non-deterministic.

Replace with explicit field-wise (de)serialization, or `#[repr(C, packed)]` plus a `to_bytes`/`from_bytes` pair.

### PD-7 — The global alert model has three independent defects 🟠

The stealth core does not hold together:

1. **`AlertChase` is cancelled on the detection frame** (`entities.rs:1350`, `1504`, `1563`). `is_in_alert` is sampled *before* the sentry loop, but `alert_state` isn't set to `Alert` until `trigger_alert()` *after* the loop. So on the frame a sentry first spots the player, line 1504 immediately demotes it back to `Patrolling`.
2. **The alert can never expire while detected** (`entities.rs:579-587`). `trigger_alert()` resets the timer to 600 *every frame*, and it is called every frame the player is in a beam. The `Alert(600) → Caution(300) → Sneaking` decay is unreachable while any detection source is live.
3. **Alert decay hardcodes the Act-1 BGM** (`entities.rs:1345`) for all nine acts that use `update_act1`. Expiring an alert in Act 2-2 or 4-2 hard-switches to the Act-1 Stealth track and leaves it there for the rest of the stage.

### PD-8 — Vision occlusion is unsound 🟠

```rust
// entities.rs:1447-1451
let mid_x = (s.x + player_x) / 2;
let mid_z = (s.z + player_z) / 2;
let obstructed = level.is_solid_at(mid_x, mid_z, false);
```

A single sample at the segment midpoint. Where a wall row has a one-tile gap (as in Act 1-2), guards see through it. This is *the* core stealth mechanic and it is a midpoint test. A DDA march over the 24×24 grid is cheap on this hardware.

### PD-9 — Save integrity has no validation and an unusable error path 🟠

- `is_valid()` (`save.rs:214-216`) checks magic, version, and checksum — and nothing else. `selected_costume`, `language`, `pal_mode` can be 0–255 and flow straight into a 3-way costume cycle (`game.rs:81`) and a 5-way language match (`game.rs:83`).
- The checksum is a **commutative sum** (`save.rs:189-212`), so any permutation of the summed fields produces an identical checksum; swapping `total_yabbies` with `alerts_count` is undetectable.
- `best_time_seconds` contributes only its low 16 bits (`save.rs:197`) while every other multi-byte field contributes both halves.
- `SaveStatus::LoadError` is declared (`save.rs:284`) and **never constructed**. A corrupt save is reported to the player as `LoadNotFound` — silently starting a fresh campaign that overwrites it on first save.
- `Err(_) => { continue; }` (`save.rs:331-337`) swallows `NotFormatted`/`Corrupt`/`Protocol` identically to `NoCard`, so the player is told "NO MEMORY CARD FOUND" for a corrupt card.

### PD-10 — Three protective anti-patterns 🟢

**`#![allow(dead_code)]` at `main.rs:8`.** Crate-wide, on a binary with no external consumers. It hides ~12 confirmed write-only fields (`entities.rs` `angle`, four `max_health`s, `target_lane`, `spark_timer`, `river_distance`; `save.rs` `highest_score`, `total_yabbies`, `alerts_count`, `best_time_seconds`, `best_codename`). Remove it and work the warnings.

**`u8 frame` counter** (`game.rs:51`, `130`). Wraps every 256 frames — 4.27 s. The renderer uses `(frame / N) % 2` for dozens of blinks; for any `N` not dividing 256 (`/3`, `/6`, `/12` are all used) the wrap inverts phase for one frame, flickering the sentry "!" marker, boss beacon, and title pulse. `u32` costs 3 bytes and removes the bug class.

**`mod` vs `pub mod` inconsistency** (`main.rs:12-24`). `save`, `dualshock`, `texture`, and `video` are `pub` inside a closed `no_main` binary crate, which reads as if they're a library API.

---

# ⚙️ 2. Senior Engineer — Hardware, Performance & Stability

*Focus: GTE/GPU correctness, RAM/VRAM/SPU budgets, CD/MDEC streaming, DualShock, draw budget.*

### SE-1 — DualShock rumble is inert: wrong `0x4D` payload 🔴

```rust
// dualshock.rs:101-105
// Byte 2 = 0x00 (map small motor to byte index 0 of poll payload)
// Byte 3 = 0x01 (map large motor to byte index 1 of poll payload)
transaction(port2, [0x4D, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]);
```

Per the PSX pad spec, `0x4D` takes `[0x4D, 0x00, num_motor_bytes, on_off_flags, …]` — **byte 2 is the *count* of motor bytes to read from the poll stream**, and byte 3 is their on/off mask. Writing `num = 0x00` tells the pad "there are no motor bytes", so the motor bytes faithfully transmitted at `dualshock.rs:172-174` are discarded. The correct payload is `[0x4D, 0x00, 0x02, 0x03, 0xFF, 0xFF, 0xFF, 0xFF]`.

The in-code comment asserts a mapping semantic the command does not have. **The entire rumble subsystem is currently non-functional**, and the README advertises "dual-motor vibration feedback".

### SE-2 — Title background wraps: right 64 columns are a duplicate of the left 64 🔴

```rust
// title_bg.rs:3-6
pub const TITLE_BG_VRAM_X: u16 = 640;
pub const TITLE_BG_WIDTH: u16 = 320;     // 320 > 256
// renderer.rs:2036-2038
gpu::draw_sprite_material(0, 0, 320, 240, (0, 0), material);
```

GPU texture coordinates are 8 bits wide (`psx-hw/src/gpu.rs:403`, `pack_texcoord(u: u8, v: u8, …)`) and a 15bpp tpage is 256 texels wide (`psx-vram/src/lib.rs:577-587`). The sprite samples u = 0..319; u truncates to 8 bits, so **columns 256–319 render a copy of columns 0–63**, and the uploaded image columns at VRAM 896–960 are unreachable dead VRAM.

This is visible on the very first frame the player sees. Fix: split the upload across two 15bpp pages, or upload two 256-wide halves and issue two sprites.

### SE-3 — The backface winding test can overflow `i32` and invert 🟠

```rust
// renderer.rs:823-834
if p0.sz < 20 || p1.sz < 20 || p2.sz < 20 || p3.sz < 20 { return; }
```

Two defects, both reachable:

1. `sz` is `u16` (`psx-gte/src/scene.rs:283`). For a vertex behind the camera the GTE saturates SZ large, so `sz < 20` is **false** — behind-camera geometry is *not* rejected. The 20-unit test only clips things nearly on the lens.
2. ```rust
   // renderer.rs:858-869
   let ax = p1.sx as i32 - p0.sx as i32;   // sx is raw i16 GTE output
   if ax * by - ay * bx <= 0 { return; }
   ```
   `p0.sx = -32768`, `p1.sx = 32767` → `ax = 65535` → `ax * by = 4.29e9 > i32::MAX`. With `overflow-checks` off in release, this wraps and **flips the sign of the winding test**: back faces drawn, front faces culled.

Fix: reject on view-space Z sign *before* projecting, and widen the cross product to `i64`.

### SE-4 — A single dropped CD sector permanently degrades the intro 🔴

```rust
// video.rs:324-330
if !self.prefetch_cd_batch(frame_to_show, storage) {
    self.using_cd = false;      // ← latched, never retried
}
```

From then on `video.rs:344` runs `frame_to_show % EMBEDDED_FRAME_COUNT` with `EMBEDDED_FRAME_COUNT = 16` (`video.rs:20`) against `TOTAL_FRAMES = 150` (`video.rs:19`). The 10-second cutscene collapses into a 16-frame (~1 s) stutter loop — and `VOICE_INTRO` keeps playing its full 10 s sample against it. **One transient read error degrades the game permanently for the session.**

Compounding it, the IRQ-mask restore is conditional on the same flag:

```rust
// video.rs:271-288
pub fn stop(&mut self) {
    if self.using_cd {                              // ← already false
        self.cd_reader.stop();
        psx_io::irq::set_mask(self.saved_irq_mask); // ← skipped
    }
```

`SectorReader::prepare()` (`video.rs:230`) masks CD/timer IRQs for the streaming duration. If `using_cd` was cleared early, the mask is **never restored** — SPU/CD IRQ handlers stay disabled for the rest of the process, which the `psx-rt` VBlank path depends on. Restore unconditionally.

### SE-5 — 782 KB of assets compiled into the executable on a 2 MB target 🔴

| Asset | Bytes | Site |
|---|---|---|
| `game/video_mdec.bin` | 262,144 | `video.rs:15` |
| `Videos/outro_audio.vag` | 126,192 | `audio.rs:19` |
| `Videos/intro_audio.vag` | 126,192 | `audio.rs:18` |
| `game/title_bg.bin` | 153,600 | `title_bg_data.rs:2` |
| `ui_select.psau` | 40,800 | `audio.rs:15` — **never played**, see SE-6 |
| `hit_metal.psau` | 31,536 | `audio.rs:12` — **uploaded twice**, see SE-7 |
| `swoosh.psau` | 14,256 | `audio.rs:11` — **uploaded twice** |
| 6 others | 26,280 | — |
| **Total** | **782,000** | **74.4% of the 1,050,624-byte image** |

ISO 9660 streaming is already implemented in `video.rs`. `video_mdec.bin` is a *fallback* — read only at `video.rs:344-357` after CD streaming has already failed — and it is the single largest object in the ROM. Drop it, or at minimum delete the four never-played samples.

### SE-6 — `VOICE_SELECT` is never configured; 13 call sites play SPU garbage 🔴

```rust
// audio.rs:36
pub const VOICE_SELECT: Voice = Voice::new(12);
// audio.rs:104-117 — the sfx table has 12 entries, V0..V11. No VOICE_SELECT.
// audio.rs:443
pub fn play_fanfare() { Voice::key_on(VOICE_SELECT.mask()); }
```

Voice 12 never receives a start address, volume, pitch, or ADSR. `play_fanfare()` is called from 13 sites (`game.rs:452,775,819`; `platypus.rs:524,551,581,608,831,835,841,846,853`) — **on nearly every stage clear and collectible**. Each call key-ons voice 12 with start address still `0x0000`, playing uninitialized SPU RAM as noise. The 40,800-byte `SELECT_SFX` sample is linked into the ROM and never used.

### SE-7 — 51,856 bytes of SPU RAM wasted on duplicate uploads 🟠

Five entries in the `sfx` table re-upload a blob that is already resident:

| Voice | Duplicates | Redundant bytes |
|---|---|---|
| `VOICE_SPUR` | `METAL_SFX` | 31,520 |
| `VOICE_SPLASH` | `SWOOSH_SFX` | 14,240 |
| `VOICE_ALERT`, `VOICE_CHIME`, `VOICE_VOICE` | `BEEP_SFX` | 2,032 ×3 |
| | **total** | **51,856** |

Point several voices at one `SpuAddr` and SPU consumption drops from ~367 KB to ~316 KB of the 512 KB available.

### SE-8 — SPU RAM allocation has no upper bound; decode errors are silent 🟠

```rust
// audio.rs:21, 119-176
const SPU_SAMPLE_BASE: u32 = 0x1010;
let mut next_addr = SPU_SAMPLE_BASE;
next_addr += (audio.adpcm_bytes().len() as u32 + 7) & !7;   // never checked against 0x80000
```

Today it lands at ~`0x5DE08` — it fits by coincidence. Separately, `Audio::from_bytes` has 11 rejection conditions (`psx-asset:1506-1518`); every failure is silently dropped at `audio.rs:121`, leaving the affected voice unconfigured with no diagnostic. Add a `const _: () = assert!(next_addr < SPU_RAM_END)` per upload, and surface failure counts on the existing options-screen hardware banner.

### SE-9 — No triangle budget; the pathological case is ~15× over 🔴

`renderer.rs:160-242` walks ~11 Z-rows, and *within each row* iterates all 11 entity arrays. Every quad costs 4 GTE `RTPS` calls and 6 GP0 words.

| | quads/frame |
|---|---|
| Tiles (Act 2 walls + gum trees) | ~1,320 |
| All entity arrays (pathological) | ~1,674/row → **~32,900 total** |
| Realistic single act (Act 2) | ~2,100 |
| PS1 30 fps capability (est.) | 2,000–5,000 triangles ≈ 1,000–2,500 quads |

`draw_model_box` is called **20 times for one character**. There is no primitive counter, no distance LOD, and no per-act early-out. The realistic case is already at or over budget. Add a per-frame budget with distance-based row truncation and LOD on the multi-box actors.

### SE-10 — Row-bucket depth sorting is not a depth sort 🟠

The module header (`renderer.rs:7`) and inline comments claim painter's-algorithm correctness. A `gz` bucket spans 64 world units (`TILE_SZ`), and within a bucket everything is emitted tiles-first — so an entity at the *near* edge of row N draws over a wall at the *far* edge of the same row. Separately, `draw_model_box_textured` (`renderer.rs:783-789`) emits faces in hardcoded order Top, Front, Back, Left, Right, Bottom with no per-face depth test; for any `rotate_y`-transformed box, Back is drawn before Left and Right, so the back face overwrites the side faces on roughly half of all rotations. Nothing writes a depth buffer as a backstop.

### SE-11 — Empty controller port costs ~918 K spin iterations per frame 🟠

```rust
// dualshock.rs:10, 233-241
const EXCHANGE_WAIT_SPINS: u32 = 32_768;
```

With no pad in a port, `poll_port_rumble` returns after one attempt — but each of ~7 `exchange` calls still spins the full 32,768 iterations on both the TX-ready and RX-not-empty waits, each an **uncached** SIO `STAT` read. Two ports × 7 bytes × 2 waits ≈ 918 K iterations/frame, on the order of 200 ms/frame. `game.rs:146-153` polls every frame including while paused, so **unplugging the pad mid-game hard-freezes the console**. Drop the spin count by an order of magnitude, or break early after a few hundred cycles.

### SE-12 — Port failover leaves the old pad's motors running 🟠

```rust
// dualshock.rs:71-87
if alt_s.is_connected() {
    self.active_port = alt_port;
    self.is_analog = init_dualshock_actuators(alt_port);
    return alt_s;                    // ← old port never gets motor-off
}
```

A pad mid-rumble on the abandoned port keeps spinning at the last commanded intensity for the rest of the session. `init_dualshock_actuators` sends the `0x4D` config but that maps motors, it does not stop them. Send `poll_port_rumble(!alt_port, false, 0)` before switching.

### SE-13 — `Game` on the stack against a 32 KiB reserve 🟠

```rust
// main.rs:35
let mut game = Game::new();     // Game on the STACK
```

`Game` embeds `VideoPlayer` → `SectorReader` → three `[u32; 512]` buffers (6,144 B), plus `Level` (`[CellType; 576]`) and `EntityManager` (11 fixed arrays). `Game::new()` calls `Renderer::new()` → `FontAtlas::upload`, which allocates a **16 KiB** `[u16; 8192]` stack buffer (`psx-font/src/lib.rs:844`) — while `Game` is still uninitialised on the caller's frame. Against `STACK_RESERVE = 0x8000` (`psoxide.ld:29`) the peak is ~28 KB. The SDK's own comment at `psx-font:786-789` warns about exactly this. Make `Game` a `static mut` singleton (as `video.rs:51` and `audio.rs:77` already do), or move the font upload to a `static` scratch buffer.

### SE-14 — VRAM layout is correct but unproven 🟠

The declared map is ~467 KB (45% of VRAM) with **no current overlap** — verified tpage widths against `psx-vram:577-587`. But six magic coordinates are spread across three modules with the framebuffer origin implied in a fourth:

```rust
renderer.rs:39-40    FONT_TPAGE = (320, 0),  FONT_CLUT = (320, 256)
texture.rs:8,11-12   ATLAS_TPAGE = (384, 0), ATLAS_CLUT = (384, 256)
title_bg.rs:3-6      TITLE_BG = (640, 0)
renderer.rs:65       framebuffer origin, implicitly, via FrameBuffer::new(320, 240)
```

The SDK ships exactly the tool and it is unused: `VramRect::overlaps` (`psx-vram:214`) and `first_vram_overlap` (`psx-vram:233`). One `const LAYOUT: [VramRect; 7]` with a compile-time overlap assert makes a collision a **build error** at zero image cost. The atlas CLUT at (384, 256) sits 64 px from the framebuffer B edge — the next region added there will collide silently.

### SE-15 — Audio will clip during cinematics 🟠

```rust
// audio.rs:96-97
spu::set_main_volume(Volume::MAX, Volume::MAX);              // 0x3FFF = +1.0
spu::set_cd_volume(spu::CdVolume::MAX, spu::CdVolume::MAX);  // 0x7FFF = +1.0
// audio.rs:149, 158
VOICE_INTRO.configure_sample(addr_intro, 22050, Volume::MAX, Adsr::sample());
```

Two voices at unity, main volume at unity, CD gain at unity. During the intro/outro the video voice runs at 1.0 while BGM (0.75), SFX (up to 0.5) and CD-DA all sum past full scale. The SPU saturates on the final sum, so any overlap clips audibly. Drop the VAG voices to ~0.6.

### SE-16 — Title music is a 127 ms hardware loop 🟢

`Music/title_music.adpcm` is 1,600 bytes = 100 ADPCM blocks × 28 samples ÷ 22,050 Hz = **127 ms**, hardware-looped for the entire title screen. A 127 ms loop point will read as a rhythmic stutter. `title_music.cdda` (23.6 MB) is mastered to the disc and is the intended full track; the embedded ADPCM looks like a placeholder.

### SE-17 — ISO 9660 parser can read past the sector buffer 🟢

```rust
// video.rs:130-135
if off + record_len > bytes.len() { break; }
let lba = u32::from_le_bytes([bytes[off+2], bytes[off+3], bytes[off+4], bytes[off+5]]);
```

The guard implies `off + 33 <= 2048`, which it does not. A record with `record_len = 1` at `off = 2047` passes, then the LBA decode reads 3 bytes past the `&[u8; 2048]`. Add `if record_len < 34 { break; }`. Relatedly, the root-directory lookup reads only sector 20 and uses a `starts_with` prefix match (`video.rs:105,139`), which will match `INTRO.VID;1` and a hypothetical `INTRO.VIDX`.

### SE-18 — PIO MDEC fallback can freeze for seconds 🟢

```rust
// video.rs:391-411 — up to 10,000 spins per output word
```

1,920 words × 10,000 spins = 19.2 M iterations per slice, × 20 slices = 384 M. The `output_words == 0` guard only helps on the first empty read. Cap the budget per *frame*, not per word.

---

# 🎨 3. Product Designer — UX, Feel & Player Experience

*Focus: does the game deliver on its own promises, and is it legible and fair?*

### UX-1 — The advertised unlock is unobtainable, and the game tells you so 🔴

```rust
// game.rs:791-797
let time_s = self.mission_stats.play_time_frames / 60;
let codename = Codename::evaluate(
    self.mission_stats.alerts_count, self.platty.total_damage, time_s, self.platty.takedowns);
// game.rs:802-804
if codename == Codename::BigPlatypus { self.save_data.camo_unlocked = 1; }
```

`mission_stats.play_time_frames` is incremented at `game.rs:706` and reset only by New Campaign — **not in `load_act`**. So the *final ranking* uses whole-campaign totals while the *stage clear card* correctly uses per-stage `stage_time_frames` (`game.rs:902`). Two contradictory clocks for the same quantity.

`save.rs:110` requires `alerts == 0 && damage == 0 && time_s <= 420` — **the entire 12-stage campaign in under 7 minutes with zero alerts and zero damage.** Effectively impossible. `camo_unlocked` is set to 1 *nowhere else in the crate* (verified by grep), so the Stealth Camo costume — which the options screen advertises as **"Camo: Rank S."** (`renderer.rs:2291`) — is permanently locked, while the Tuxedo is granted unconditionally at `game.rs:800`.

This is the single most damaging product bug in the build: a visible, described reward that can never be earned, plus a UI that lies about how to get it.

### UX-2 — Laser tripwires are decorative 🔴

`CellType::LaserTripwire` (`level.rs:26`) is not in `is_solid` (`level.rs:33-40`), not matched in `get_surface_and_noise` (`platypus.rs:177-189`), and never read in `entities.rs`. Its only consumers are the draw call (`renderer.rs:519`) and a radar blip (`renderer.rs:1866`). Eight tripwires are placed across Acts 1-1, 1-2, and VR-04; the level subtitle for 1-2 promises "dodge laser tripwires" (`level.rs:103`). **Walking through one does nothing at all.**

Worse, three of Act 1-2's six tripwires are silently destroyed by level generation ordering:

```rust
// level.rs:407-409  (written first)
self.set_cell(11, 6,  CellType::LaserTripwire);
self.set_cell(11, 11, CellType::LaserTripwire);
self.set_cell(11, 16, CellType::LaserTripwire);
// level.rs:430-434  (runs later, same generator, last-write-wins)
self.set_cell(11, z, CellType::MetalGrate);   // clobbers all three
```

Half the "LASER GRID MAZE" is missing, and the level generator's pass ordering is load-bearing and undocumented — a latent hazard for every one of the 16 generators.

### UX-3 — 19 HUD and menu strings run off the 320 px screen 🔴

The font is 8 px/char and `draw_text` (`psx-font:967-969`) has no bounds check. Confirmed offenders:

| Location | String | Right edge |
|---|---|---|
| `renderer.rs:2140` | `"DPAD: SELECT \| START / CROSS: CONFIRM"` (38 ch @ x=48) | **352** |
| `renderer.rs:2508` | `"THANKS TO: PS1 HOMEBREW & RUST EMBEDDED COMMUNITY"` (47 @ x=24) | **400** |
| `renderer.rs:2509` | `"TACTICAL ESPIONAGE ACTION HOMAGE TO KOJIMA PRODUCTIONS"` (50 @ x=24) | **424** |
| `renderer.rs:2686` | `"DPAD: SELECT \| CROSS: DEPLOY \| CIRCLE: BACK"` (45 @ x=22) | **382** |
| `renderer.rs:2040` | `"PLATTYPUS : TACTICAL ESPIONAGE"` (33 @ x=60) | **324** |
| `renderer.rs:2402` | `act.title()` e.g. `"ACT 3-3: ANTENNA TOWER (SNIPER KOOKY)"` | **336** |
| `renderer.rs:2276` | `"SIMULATEUR ENTRAINEMENT VR"` (25 @ x=168) | **368** |
| `renderer.rs:2295` | `"Experience Plattypus in early 90s PS1 vectors."` (45 @ x=26) | **386** |
| `renderer.rs:2303` | `"Switch between 60Hz NTSC and 50Hz PAL modes."` (42 @ x=26) | **362** |
| `renderer.rs:2291` | `"Tuxedo: Beat campaign. Camo: Rank S."` (40 @ x=26) | **346** |

The localisation strings are the worst offenders because they were never measured — switching to German or French (`renderer.rs:2059-2065`, `2092-2118`) overflows immediately. There is no `text_width` call anywhere in the game. One helper fixes all of them:

```rust
// Measure once, clamp once.
let w = self.font.text_width(s);
let x = x.min(SCREEN_W as i32 - w as i32);
```

Note also `renderer.rs:1908-1911` hardcodes `msg_len * 8` for the prompt box width instead of calling `text_width`.

### UX-4 — Analog movement collapses to zero on diagonals 🔴

```rust
// platypus.rs:648-665
let mag = isqrt_i32(sx_i32 * sx_i32 + sy_i32 * sy_i32);   // mag <= 127, correct
let speed = (max_speed * mag) / 127;                     // first /127: fine
let speed = speed.max(1);
self.vx = (sx_i32 * speed) / 127;                        // second /127: WRONG
self.vz = (-sy_i32 * speed) / 127;
```

`speed` is a scalar but is applied per-axis, re-normalizing against 127 a second time:

| Input | mag | speed | vx, vz | Resultant | Intended |
|---|---|---|---|---|---|
| 45° at ~70% (`sx=sy=63`) | 89 | 2 | 0, 0 | **0.00** | ~2.5 |
| 45° at full (`sx=sy=89`) | 89 | 3 | 2, 2 | 2.83 | 4.0 (~71%) |

At moderate diagonal deflection the player **does not move at all**. The digital D-pad path sets `vx`/`vz` directly, so this reproduces as "analog feels broken, digital is fine". `speed.max(1)` cannot rescue it — it operates before the truncating division.

Derive direction from the `atan2` already computed at `platypus.rs:645` as `self.angle`, and scale by `max_speed` directly.

### UX-5 — A motionless-box invisibility exploit falls out of UX-4 🔴

Because `vx == 0 && vz == 0` at diagonal analog, `game.rs:719` computes `player_moving = false` while `player_in_box` is true. Every concealment check is then:

```rust
// entities.rs:1392, 1415, 1467
let box_safe    = player_in_box && !player_moving;
let box_ignored = player_in_box && !player_moving;
```

So a player in a cardboard box holding the stick diagonally is **invisible to searchlights, drones, and sentry vision cones** while still emitting loud footsteps at full volume. `player_moving` should derive from the same input-intent signal, not from post-truncation velocity.

### UX-6 — Unaware guards melee you 🔴

```rust
// platypus.rs:859-869
if s.active && s.stun_timer == 0 {
    if dx < 28 && dz < 28 && s.attack_cooldown == 0 {
        s.attack_cooldown = 45;
        self.take_damage(1);
```

No `see_player`, no `state == AlertChase`, no check on `entities.alert_state`. A sentry rendering the yellow "?" — or nothing at all (`renderer.rs:1474-1494`) — still hits for 1 damage every 45 frames at 28 units. **Sneaking right up to a patrolling guard is strictly worse than staying away**, which inverts the core fantasy. Gate damage on an alert/see condition.

### UX-7 — The final boss has no phases, and three fields prove it was planned 🔴

```rust
// entities.rs:1889-1914
self.boss_excavator.state_timer = self.boss_excavator.state_timer.wrapping_add(1);
```

`state_timer` is incremented and never read. `shields_down` (`entities.rs:303`) is initialised at 321 and never touched. `engine_hp: [u8; 3]` (`entities.rs:304`) is set to `[2, 2, 2]` at 322 and 1179 and **never read or modified**. The Act 4-3 subtitle promises "Overload Dr. Cane Toad's amphibian excavator" and the level provides a nursery to protect, but the climax is a flat 4-HP stationary target with a claw sweep and a slime mortar. The climax does not earn the campaign.

### UX-8 — Enemies that don't patrol 🟠

**Drones orbit a radius of 2 world units** (`entities.rs:1408-1410`):

```rust
d.angle = d.angle.wrapping_add(1);
d.x += (cos_1_3_12(d.angle) as i32 * 2) >> 12;
```

Values in `-2..=2` — a 2-unit jitter around the spawn point. The 6 drones placed in Acts 1-2 and 3-2 are stationary hazards, not patrols.

**Jetski `target_lane` is never read** (`entities.rs:225`). The boss ping-pongs between two hardcoded Z positions (`entities.rs:1793-1797`).

**The river freezes when the jetski dies** (`entities.rs:1782-1785`): `update_act2_3_boss` early-returns before `update_act2()`, so all river obstacles stop for the rest of the stage — while the player is still often running toward the exit.

### UX-9 — Drones/chase state glitches and the boss orbit stutters 🟠

**Boss intro "orbit" is a 16-step stutter** (`game.rs:669-674`):

```rust
let ang = ((*timer as u16) * 16) & 0x0FFF;
let sin_v = sin_1_3_12(ang) as i32;
```

`sin_1_3_12` masks to 256 units per revolution, but `& 0x0FFF` implies 4096. The effective angle is `(timer & 0x0F) * 16` — **16 distinct camera positions**, repeating 15 times over the 240-frame cutscene. This is meant to be the most cinematic moment in the game.

**Chase state flickers** — see PD-7.1. The visible effect is the "!" icon dropping out for a frame at the moment of detection.

### UX-10 — Nothing is scored where the player can see it 🟠

Five save fields are written and never read anywhere in the tree (verified by grep): `highest_score`, `total_yabbies`, `alerts_count`, `best_time_seconds`, `best_codename`. `best_codename` is populated at `game.rs:810-812` and **never displayed**.

There is no leaderboard, no best-time display, no collectible total, no rank history, and no end-of-run summary beyond the 12 codenames. For a game whose central reward loop is a 12-tier ranking system, the player gets one rank, once, with no record of what they did or what to beat.

### UX-11 — Stage Select shows no progression state 🟠

```rust
// renderer.rs:2621
pub fn draw_stage_select_menu(&self, selected_stage: usize, _unlocked_act: u8)
```

`game.rs:301` passes the unlock data; the renderer discards it. All 12 stages render identically — no lock icons, no clear marks, no best times, no medal. The menu is only reachable when `unlocked_act >= 11` (`game.rs:187`), so the data is always "all open" — but a Stage Select with no stage state is just a stage list.

### UX-12 — No difficulty option; ranking is the only difficulty axis 🟠

`grep -rni difficulty game/src/` returns **nothing**. The options menu has exactly five rows (`game.rs:345-405`): costume, wireframe, language, video standard, screen offset. The 12-rank codename system is the sole skill-expression mechanism, and because of UX-1 its top tier is unreachable. There is no way to make the game easier for a first-time player, and the 4 VR training missions (a genuinely good onboarding idea) are gated behind campaign progress or the title menu.

### UX-13 — Silent failures on every options-menu change 🔴

The memory-card status OSD lives inside the `GameState::Playing` arm (`game.rs:830-852`), but `save_to_slot1` is also called from all five Options-menu rows (`game.rs:359, 367, 379, 392, 404`). **Every options change on a cardless console fails completely silently** — the value changes in memory, the write fails, and the player is told nothing until power-off.

Related: `game.rs:859` computes `memcard_ok` from `memcard.status`, which `Game::new()` set to `LoadNotFound` with a 90-frame timer. `LoadNotFound` isn't in the allow-list, so for the first ~1.5 s after boot the SCRIBE call-in plays "no memory card found" *on a healthy card*.

### UX-14 — "Video Standard" and "Screen Offset" options don't fully work 🟠

**Video Standard never changes the video standard** (`game.rs:382-394`). `apply_display_offset` only issues `h_display_range`/`v_display_range`; the PAL bit lives in `gp1::display_mode`, written exclusively by `gpu::init()` (`psx-gpu/src/lib.rs:137`) — called once from `Renderer::new()`. On real hardware the option does nothing. The on-screen region label (`renderer.rs:2281`) also re-derives from `detect_console_region()` rather than the player's choice.

**All frame→second conversions hardcode 60 Hz** (`game.rs:424, 791, 902`) despite PAL support at `save.rs:45-56` and an explicit 50 Hz option. On a PAL console every displayed time, the stored `best_time_seconds`, and all 12 codename thresholds are **20% too generous** — the PAL player is being graded on a different difficulty.

**Screen offset** only exposes the vertical axis (`game.rs:396-405`); `screen_offset_x` is loaded and applied but has no editor and no writer.

### UX-15 — Memory management: save, continue, and new campaign are all wrong 🟠

- `memcard.status` shows a false "no card" for 90 frames after boot (above).
- `SaveStatus::Saving` (`save.rs:315`) is always overwritten synchronously before any reader can observe it, so the "SAVING..." OSD at `game.rs:831-834` is dead code.
- `SaveStatus::LoadError` is never constructed (`save.rs:284`), so a corrupt save is reported as "not found" and silently overwritten.
- "NEW CAMPAIGN" doesn't reset the save (PD-3), so a completed save cannot be replayed from the beginning.
- No `Full` card handling exists anywhere (prior QA-6) — a full card produces a generic `SaveErrorFailed`.

### UX-16 — Small polish items 🟢

- **Em-dash dropped mid-sentence** (`codec.rs:311`): `"It's Sniper Kooky—an elite"`. The font is ASCII-only (128 glyphs), so `glyph_uv` returns `None` and the character vanishes leaving an 8 px gap. This is the only non-ASCII character in any user-facing string.
- **Stage clear reports 5 ranks** (`MANUAL.md:213-219`) but the game awards 12, with wrong thresholds (see MK-4).
- **Controller deadzone is duplicated and half-applied** (`dualshock.rs:30` vs `platypus.rs:622`). `DualShockController::deadzone` is written once and never read; the gameplay code builds a second, independent `Deadzone::new(18)`. Only the **left** stick is gated — the right stick, used for aiming, has no deadzone at all.
- **No in-game brightness/gamma control**, and the only CRT adjustment is a 33-step vertical offset. Many PS1 games shipped a brightness slider; on modern displays this is the single most requested option.

---

# 📣 4. Marketing — Positioning, Presentation & Distribution

*Focus: what a buyer sees, what the packaging claims, and whether the docs match the build.*

The engineering achievement here is genuinely notable — a 3D tactical stealth game in bare-metal Rust, with hardware MDEC streaming and an animated BIOS memory-card icon, is a legitimate hook. But the material a customer actually touches is inconsistent with the build, and several claims are demonstrably false.

### MK-1 — The retail title is two different products 🔴

| "ACTION" | "OPERATION" |
|---|---|
| `README.md:1` | `MANUAL.md:1-2` |
| `PLAN.md:1` | `JEWEL_CASE_SPEC.md:14, 25, 49` |
| `INTRO.md:98` | `DISC_SURFACE_SPEC.md:24` |
| `OUTRO.md:1` | |

Every gameplay and promotional document says **Tactical Espionage ACTION**. Every packaging document says **Tactical Espionage OPERATION**. To a shelf browser or a search query these are different products. Pick one and sweep all seven files.

### MK-2 — The README links to files that don't exist in a fresh clone 🔴

```markdown
README.md:76-78
- `dist/plattypus.exe`
- `dist/plattypus.bin`
- `dist/plattypus.cue`
```

`dist/` is gitignored (`.gitignore:3`). `README.md:85` hyperlinks `dist/plattypus.cue` as though it were a repository artifact. **Every link is dead for every fresh clone.** Ship a release artifact (or a tagged release link) instead.

### MK-3 — Feature claims that the build does not honour 🔴

| Claim | Source | Reality |
|---|---|---|
| "dual-motor vibration feedback" | `README.md:35` | **Rumble never actuates** — wrong `0x4D` payload (SE-1) |
| "dodge laser tripwires" (Act 1-2 subtitle) | `level.rs:103`, 8 placed tripwires | **Purely decorative** (UX-2) |
| 12 codename ranks | `save.rs:58-72` | Rank S **unreachable** → advertised Camo unobtainable (UX-1) |
| "Video Standard: NTSC / PAL" | `game.rs:383` | **Never re-programs the GPU** (UX-14) |
| 5-language support incl. 日本語 | `MANUAL.md:237` | No CJK font ships; the string is `"NIHONGO (ROMAJI)"` (`save.rs:38`) |
| 128-byte save payload | `MANUAL.md:243` | That's the read buffer; `size_of::<SaveData>()` is **48** bytes |

### MK-4 — The manual documents a build that no longer exists 🔴

`MANUAL.md` was never updated after the 12-rank expansion (commit `4348bcc`):

| Manual says | Code says |
|---|---|
| 5 codenames (`MANUAL.md:213-219`) | 12 (`save.rs:58-72`) |
| TASMANIAN DEVIL: **6+** takedowns | `takedowns >= 10` (`save.rs:116`) |
| SLY POSSUM: under **450** seconds | `time_s <= 600` (`save.rs:122`) |
| "DUCKBILL ROOKIE (**Rank C**)" | "JUNIOR OPERATIVE (**RANK D**)" (`save.rs:105`) |
| Boss 2 is "**Ranger Dave**" (`MANUAL.md:178,192`) | "**Park Ranger Jet Ski**" (`level.rs:98`) |

And it contradicts itself within 71 lines: `MANUAL.md:135` says **CROSS** executes CQC (the code is `just_square`, `platypus.rs:228` — and `MANUAL.md:64` itself agrees), while `MANUAL.md:140` says **CIRCLE** conceals the Bill Box (the code is L1, `platypus.rs:231` — and `MANUAL.md:66` itself agrees). **Both errors are in the same document that has the right answer 70 lines earlier.** A player following the manual will press the wrong buttons.

### MK-5 — Four different serial numbers for one SKU 🔴

`DISC_SURFACE_SPEC.md:30,47` → `BASLUS-00001`
`JEWEL_CASE_SPEC.md:25` → `SLUS-00001`
`JEWEL_CASE_SPEC.md:49` → `SLES-00001`
`MANUAL.md:3` → `BASLUS-00001 / SCES-00001`

Also unstated: `JEWEL_CASE_SPEC.md` gives 6 mm spines for NTSC (line 20-22 decomposes 150 mm as 6+138+6) and 6.5 mm for PAL (line 48) without saying that's intentional.

And the disc spec's track map is decorative: `DISC_SURFACE_SPEC.md:52-53` claims Track 1 runs to `03:10:60` and Track 2 to `07:28:45`, but the actual `dist/plattypus.cue` puts the CD-DA start at `01:29:68`. **The spec was never derived from the mastered image.**

### MK-6 — No licence, no provenance, no release trail 🔴

- **No `LICENSE` at the repo root.** `game/Cargo.toml:5` declares `GPL-2.0-or-later`. For a project pursuing commercial physical distribution (`PLAN.md:126-136`), shipping without the licence text is a legal defect, and GPL-2.0 on a commercial physical release carries obligations worth confirming.
- **No asset provenance for ~28.6 MB of third-party media.** `Music/` contains `Below_the_Reeds` and `King_of_the_Yarra` as `.mp3`/`.mp4`; `Videos/` contains `Platypus_Intro.mp4` and `Platypus_outro.mp4`. None are referenced by any build step, and none has a documented licence. Meanwhile `DISC_SURFACE_SPEC.md:50` already asserts a Sony licence on the disc face. A commercial release cannot ship unlicensed music.
- **No `CHANGELOG.md`, no `CONTRIBUTING.md`, no `SECURITY.md`, no CI badge.** There is no CI, so a green-looking repository gives no signal about whether it builds.
- **The submodule URL is a local filesystem path** (`.gitmodules:3`, `url = ./psoxide`). **A fresh clone cannot initialise it.** `README.md:3` already links the real upstream — `https://github.com/EBonura/PSoXide` — which should be the `url`.

### MK-7 — Zero discoverability assets 🔴

The repository has one screenshot (`Images/frontscreen.png`, 587 KB, committed). For a project with a stated goal of commercial release there is no trailer, no gameplay GIF set, no comparison imagery, no "what makes this different from Metal Gear Solid" framing beyond a single README paragraph, and no itch.io/Steam/Demo page. The strongest marketing asset this project has — *a stealth game written in bare-metal Rust, running on 1994 hardware, with real MDEC cinematics* — appears nowhere in the README as a headline.

### MK-8 — Stale planning documents read as a live status board 🟠

- `PLAN.md` still describes the game as a "4-stage technical prototype" (lines 7, 88). All 12 acts and 4 VR missions ship. It lists a codename "**Puddle Tadpole**" (line 119) that exists nowhere in the source, and promises "**15** standalone VR training levels" (line 121) against 4 that exist.
- `TODO.md` has **every single box checked** (132/132), including two "Active Issues" bug reports (lines 9-18) that are presented as open.
- `REVIEW.md` (this file, previously) was 24 KB of AI-generated audit in the repo root, half-obsolete, asserting a resolution status that was false.

None of these are wrong as history. They're wrong as *current documentation* — a reader arriving today concludes the project is finished and verified, which it is not.

### MK-9 — Ending credits need a verification pass 🟢

Commit `c25cea4` added ending credits citing "PS1 HOMEBREW & RUST EMBEDDED COMMUNITY" (`renderer.rs:2508-2509`). Both lines overflow the screen (UX-3). Confirm every named entity actually consented to being named, and that the PSoXide authors are credited in the credits roll rather than only in `README.md:3`.

---

# 🧪 5. Testing Developer — Coverage, Architecture & Automation

*Focus: what the tests actually prove, what they miss, and whether anyone runs them.*

### QA-1 — The test suite proves nothing about the shipped binary 🔴

`tools/test_game_logic/Cargo.toml:6` has an **empty `[dependencies]`** and zero path deps. `main.rs` re-declares the game's types from scratch:

| Item | Game (source of truth) | Test harness (copy) |
|---|---|---|
| `Codename` | `save.rs:58-136` | `main.rs:8-86` |
| `SaveData` | `save.rs:138-160` | `main.rs:88-110` |
| `SaveData::new/compute_checksum/is_valid` | `save.rs:162-217` | `main.rs:112-167` |
| `Act` + 6 methods | `level.rs:48-205` | `main.rs:169-244` |
| `CellType` + `is_solid`/`is_water` | `level.rs:15-46` | `main.rs:247-275` |

A line-level diff of `SaveData` against the game returns exactly **one** difference, and it is a comment. Everything matches — today. **Nothing enforces that.** Editing `save.rs` cannot fail `make test`.

Worse, several suites test **local reimplementations** rather than game logic:

| Suite | What it actually tests |
|---|---|
| Oxygen (QA-3) | A local `sim_air`/`sim_health` closure pair (`main.rs:366-397`) |
| Box (QA-5) | A bare `if has_box && on_ground && !in_water` (`main.rs:400-430`) |
| Boss retry (QA-2) | `MockMechBoss`'s own constructor (`main.rs:435-458`) |
| Rapids (QA-4) | A local `is_solid_at` closure, not `Level::is_solid_at` (`main.rs:489-492`) |
| PAL (QA-8) | Two closures differing in `/60` vs `/50` (`main.rs:534-537`) — never calls `detect_console_region()` |

**These are tautological with respect to the shipped game.** They would all still pass if `platypus.rs` were rewritten to be completely wrong. The two suites that *do* have value — save checksum round-trip and the 12 codename thresholds — test duplicated constants.

### QA-2 — Zero CI for this repository 🔴

There is no `.github/` at the repo root. The only workflow anywhere is **`psoxide/.github/workflows/ci.yml`**, which lives *inside the submodule* and runs the **SDK's** `make check/test/lint/hello-tri` over the SDK's own crates. It never touches `game/` or `tools/test_game_logic/`.

`make test`, `make exe`, and `make disc` are **never executed automatically on any push or PR.** Because `psoxide` is a nested repo with its own `.github/`, any tool probing the tree will report "this project has CI" — which is false, and is worse than having none.

Also missing: a `rust-toolchain.toml` at the repo root or in `game/`. The only pin is `psoxide/rust-toolchain.toml` (`nightly-2026-03-25`), whose own comment says *"update the downstream game repos' toolchain files with it"*. The downstream file does not exist, so the game builds under whatever nightly the developer happens to have. This machine has `1.100.0-nightly`; the SDK pins `2026-03-25`. Silent drift.

### QA-3 — No `#[test]` anywhere; no property-based or fuzz testing 🔴

`rg "#\[test\]|#\[cfg\(test\)\]|proptest|quickcheck|fuzz" game/ tools/` returns **zero matches**. The 63 assertions live in a hand-rolled `fn main()` with bare `assert!`. Consequences:

- No `cargo test` integration, no test selection, no parallel execution, no `--nocapture`.
- A failing assertion aborts the binary with no test-name context beyond a `println!` that may not have flushed.
- The suite numbering **skips QA-6** while the header claims "11/11 test suites" — so the numbering itself has drifted from the tracking doc.

### QA-4 — Untested surface, in priority order 🔴

| Untested | Location | Risk |
|---|---|---|
| `SaveData` field-range validation | `save.rs:214-216` | Crafted/rotted save drives costume & language indices to nonsense |
| Corrupt-save path | `save.rs:390-408` | `LoadError` never constructed; data silently overwritten |
| `MemoryCardManager` slot fallback | `save.rs:320-353` | Untestable as written — depends on `psx_mc::HardwareCard` |
| Full-card handling | absent | Prior QA-6, still unimplemented |
| `Language` enum | `save.rs:12-41` | 5-way selector, zero coverage |
| `detect_console_region()` | `save.rs:45-56` | BIOS ROM read, hardcoded address, no fallback |
| `Level::is_solid_at` / `is_water_at` | `level.rs:251-260` | The real collision entry point |
| `Act::title/subtitle/stage_label/chapter` | `level.rs:79-150` | 4 of 8 methods; drives every HUD string |
| Sentry 4-state FSM | `entities.rs:1350-1564` | The three PD-7 defects are all here |
| Boss state machines | `entities.rs:1598-1914` | 4 bosses, zero coverage |
| Level generators | `level.rs:389-754` | 16 generators; the Act 1-2 tripwire clobber lives here |
| `video.rs` ISO 9660 parser | `video.rs:99-150` | Has a real OOB read (SE-17) |
| `audio.rs` SPU allocator | `audio.rs:119-176` | No upper bound; fits by accident |

### QA-5 — Property-based testing is the highest-leverage addition 🟠

`Codename::evaluate(alerts, damage, time_s, takedowns)` is a pure function over four small integers with 12 outcomes and hand-tuned thresholds — the ideal target for an exhaustive sweep. A single `for alerts in 0..=32 { for damage in 0..=32 { for time in (0..=900).step_by(5) { for takedowns in 0..=20 { ... } } } }` would have **caught UX-1 immediately**: it would prove that `BigPlatypus` is reachable, and if not, exactly why.

The same technique applies to `SaveData::compute_checksum` (currently an order-independent sum — QA/PD-9) and `Act::from_u8` (already has a `0..=255` sweep, which is the right pattern; the other enums should get one).

### QA-6 — Make the tests test the real code 🔴

Three options, in increasing order of effort:

1. **Add a `[lib]` target to `game/Cargo.toml`.** The crate is `[[bin]]`-only today (`game/Cargo.toml:9-11`), which is the structural reason the harness can't import anything. A `src/lib.rs` exposing `save`, `level`, and the pure parts of `platypus`/`entities` would let the test crate depend on the real modules. Note `panic = "abort"` in the release profile doesn't affect this — tests run in the dev profile.
2. **Extract a `plattypus-core` `no_std` crate** holding `SaveData`, `Codename`, `Act`, `CellType`, and the pure gameplay predicates, with the binary depending on it. This is the cleanest and also enables the "is this PS1 hardware-specific?" question to be answered by directory layout.
3. **Leave it** and mark the suite honestly as "specification tests, not implementation tests" — in which case `README.md` and the `make test` help text must say so, and the CI badge must not imply coverage of `game/src/`.

Right now the suite is silently option 3 while presenting as option 1.

### QA-7 — Build reproducibility hazards 🟠

- `game/.cargo/config.toml` sets `target = "mipsel-sony-psx"`, but it **only applies when cwd is `game/` or below**. Running `cargo build --manifest-path game/Cargo.toml` from the root silently builds for the **host**. The `Makefile` gets this right (`cd $(GAME_DIR) && cargo build`, line 25) — and indeed `game/target/x86_64-unknown-linux-gnu/` exists, evidence the footgun has already been stepped on.
- No `--locked` anywhere in this repo's `Makefile`, so dependency drift in either lockfile goes unnoticed.
- `game/build.rs:8` hard-expects `game/` to sit directly under the repo root (`.expect("game crate must live in repo root")`).
- `Makefile:45,47` hardcodes `/home/tonym/Downloads/DuckStation-x64.AppImage` — a developer's absolute home path committed into the build system.
- `make disc` has no dependency on its media inputs existing; it fails late inside `mkisopsx` instead of with a clear message.

### QA-8 — Repo hygiene: 66.6 MB tracked, 28.6 MB of it dead 🟠

| Item | Size | Note |
|---|---|---|
| `Music/*.mp3` + `*.mp4` | **28.6 MB** | **Zero build references.** `rg "\.mp3\|\.mp4"` across `Makefile`, `tools/`, `game/src/` → one *comment* hit at `audio.rs:130` |
| `Music/title_music.cdda` | 23.6 MB | Build input (`Makefile:35`) |
| `Videos/*.mp4` | 8.8 MB | Encoder *inputs*, not build inputs |
| `Videos/*.STR` / `*.VID` / `*.vag` | ~9.1 MB | Build inputs ✅ |
| `dist/video-probe.*` | ~11 MB | **Orphans** from Sep 25; no Makefile target produces them, `.iso` especially |

No `.gitattributes`, no LFS. `Music/Credtis-Below_the_Reeds.mp4` carries a filename typo (missing "i"). `create_str.py` sits at the repo root rather than beside `tools/encode_mdec_video.py`. `.opencode/` is untracked *and* un-ignored at root, so `git status` is permanently dirty and agent state is one `git add -A` away from being committed.

### QA-9 — What the suite should assert about the current defects 🟠

These are cheap to add once the real modules are importable, and each one is currently a live bug:

1. `unlocked_act` never exceeds 11 after *any* save path — including the CODEC path (`game.rs:866`), which is the one place the clamp is missing.
2. `Codename::evaluate` returns `BigPlatypus` for at least one input reachable in a real playthrough.
3. `unlocked_act == 11` does not make the title screen show STAGE SELECT.
4. "NEW CAMPAIGN" produces a save where `unlocked_act == 0`.
5. `SaveData` with a valid checksum but out-of-range `selected_costume` / `language` / `pal_mode` is rejected or clamped.
6. A corrupted save sets `LoadError`, not `LoadNotFound`.
7. `sin_1_3_12`-based camera orbit produces ≥ 60 distinct positions over 240 frames.
8. Analog input at 45°/70% produces non-zero velocity.
9. Every `LaserTripwire` cell in a generated level is reachable and adjacent to a walkable tile.
10. `size_of::<SaveData>()` equals the sum of its field sizes (no padding on disk).

---

# 📋 Merged Priority Plan

## P0 — Ship blockers (fix before anyone plays a build)

| ID | Issue | Why P0 |
|---|---|---|
| SE-1 | Rumble never actuates (`0x4D` payload) | An advertised hardware feature is 100% non-functional |
| SE-2 | Title bg UV wraps at 256 | Corrupt art on the **first frame the player sees** |
| UX-1 | Rank S unreachable → Camo unobtainable | Advertised reward can never be earned; options screen says how |
| QA-1/PD-1 | `unlocked_act` unclamped on the CODEC path | Corrupts the save; on a fresh card, one VR sim + a CODEC save falsely marks the campaign complete |
| SE-6 | `VOICE_SELECT` never configured | 13 sites play uninitialized SPU RAM — on most stage clears |
| UX-3 | 19 strings overflow 320 px | Visible defect on title, stage clear, credits, stage select, VR menu, and all 5 languages |
| MK-1/MK-2 | Two product names; README links to gitignored files | Broken identity + every README artifact link is dead on clone |
| MK-4 | Manual documents the wrong buttons and the wrong ranks | Player follows the manual, presses the wrong button |
| MK-6 | `.gitmodules` `url = ./psoxide` | **A fresh clone cannot build the project** |
| PD-6 | Uninitialized padding bytes written to the memory card | Non-deterministic saves + stack residue on cartridge media |

## P1 — Quality-gate issues (fix before external review)

| ID | Issue |
|---|---|
| SE-4 | CD hiccup latches `using_cd = false` → 150-frame intro becomes a 16-frame loop **and** leaks the IRQ mask for the session |
| SE-3 | Backface winding overflows `i32` → front/back faces invert |
| UX-4/UX-5 | Analog diagonal movement is zero-velocity, and that zero grants box invisibility |
| UX-6 | Unaware guards melee the player |
| PD-7 | Alert can never expire; `AlertChase` cancelled on the detection frame; wrong BGM on decay |
| PD-8 | Line-of-sight is a midpoint sample — guards see through walls |
| UX-13 | All 5 options-menu saves fail silently; false "no card" for 90 s after boot |
| PD-9 | No save field validation; commutative checksum; `LoadError` never used |
| QA-2 | No CI; no toolchain pin |
| QA-1 | Tests validate duplicated copies, not game code |
| SE-9 | No draw budget; realistic act is at/over hardware capability |
| MK-3 | Six specific README/manual claims the build does not honour |
| PD-3 | "NEW CAMPAIGN" cannot replay a completed save |
| UX-14 | Video Standard option does nothing; PAL timing 20% wrong |

## P2 — Depth and polish

| ID | Issue |
|---|---|
| PD-4 | Split `renderer.rs` (2,688 lines, 38 concerns) into `prim`/`traversal`/`tiles`/`models`/`hud`/`screens`/`lang` |
| PD-5 | Level layout logic (`gz == 18`, `gx == 4`) living in the renderer |
| UX-7 | Final boss has no phases; `engine_hp`/`shields_down`/`state_timer` are dead fields |
| UX-8 | Drones orbit radius 2; jetski `target_lane` unread; river freezes on boss death |
| UX-10 | 5 statistic fields written and never displayed; no best-time, no rank history |
| UX-11 | Stage Select renders no progression state |
| UX-12 | No difficulty option |
| SE-5 | 782 KB of embedded assets on a 2 MB target |
| SE-7/SE-8 | 52 KB of duplicate SPU uploads; no SPU RAM bound; silent decode failures |
| SE-11/SE-12 | Empty-port spin stall; port failover leaves motors running |
| SE-13 | `Game` + font buffer against a 32 KiB stack reserve |
| SE-10/SE-14 | Row-bucket "depth sort"; unproven VRAM layout |
| SE-15/SE-16 | Audio clipping in cinematics; 127 ms title-music loop |
| UX-2 | Laser tripwires decorative; 3 of 6 clobbered by generator ordering |
| UX-9 | Boss orbit is a 16-step stutter |
| MK-5 | Four serial numbers; disc track map never derived from the image |
| QA-8 | 28.6 MB of unreferenced media tracked; orphaned `dist/video-probe.*` |
| — | `PLAN.md`/`TODO.md` read as a live status board and are wrong |

## P3 — Cleanup

SE-17/SE-18 (ISO 9660 OOB read, PIO spin budget) · PD-10 (`#[allow(dead_code)]`, `u8 frame` wrap, `pub mod` inconsistency) · three dead bounds checks in `entities.rs` that give false confidence · two divergent crab-patrol implementations · duplicated camera-target math (3×) · `texture.rs` `CarGrill` never referenced · em-dash dropped mid-sentence · `EXCHANGE_WAIT_SPINS` tuning · `.opencode/` gitignore · `create_str.py` location · the 7 stale local branches (`MGS`, `MGS2`, `MGS3`, `MGS4`, `testyviddy`, `video`, `video2`).

---

## Appendix — Verification performed

Everything above was checked against the tree at `832a502`, not inferred from the previous revision of this document. Specifically confirmed by direct inspection:

- `VOICE_SELECT` is defined at `audio.rs:36` and absent from the 12-entry `sfx` table at `audio.rs:104-117`; `play_fanfare()` at `audio.rs:443` key-ons it.
- The `0x4D` payload at `dualshock.rs:105` is `[0x4D, 0x00, 0x00, 0x01, ...]` — byte 2 is `0x00`.
- `game.rs:866` writes `unlocked_act` with `.max()` and no `.min(11)`, unlike `game.rs:785` and `game.rs:887`.
- `SaveData` (`save.rs:138-160`) is `#[repr(C)]` and not packed; `magic[4] + version + unlocked_act` = 6 bytes precedes a `u32`, producing 2 padding bytes in a 48-byte struct.
- `camo_unlocked` is assigned `1` only at `game.rs:803`; no other writer exists.
- `grep -rni difficulty game/src/` returns nothing.
- `highest_score`, `total_yabbies`, `alerts_count`, `best_time_seconds`, and `best_codename` have no read sites outside `save.rs` and their own write sites.
- `tools/test_game_logic/Cargo.toml` has an empty `[dependencies]`.
- No `.github/` directory exists at the repo root; the only workflow is `psoxide/.github/workflows/ci.yml`.
- `.gitmodules` contains `url = ./psoxide`.
- No `rust-toolchain.toml` exists outside the submodule.
- `make test` passes: 11/11 suites, 63 assertions, exit 0.
- `cargo build --release` in `game/` succeeds, producing a 1,052,672-byte executable.
