# Plattypus: Tactical Espionage Action — Multi-Perspective Project Review

**Review date:** 2026-10-02
**Commit reviewed:** `9e6a575`
**Perspectives:** Principal Developer · Senior Engineer · Product Designer · Marketing · Testing

This is the third review of the project. The previous two are preserved in git
history; this document supersedes both.

> **Status: remediation in progress.** Every item in §7 was implemented and
> verified in this pass. §8 lists what is still open. §1–§6 record the findings
> and the evidence behind them, so the reasoning survives the fix.

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Principal Developer](#2-principal-developer--architecture)
3. [Senior Engineer](#3-senior-engineer--hardware-and-performance)
4. [Product Designer](#4-product-designer--ux)
5. [Marketing](#5-marketing--positioning-and-distribution)
6. [Testing](#6-testing--strategy-and-automation)
7. [What was fixed](#7-what-was-fixed)
8. [Still open](#8-still-open)
9. [Appendix: verification](#9-appendix-verification-performed)

---

## 1. Executive summary

The project is a bare-metal `#![no_std]` PlayStation 1 game in Rust, roughly
13,000 lines of game code plus a 716-line shared logic crate, built with the
in-tree PSoXide SDK. It is further along than any earlier review suggested, and
the previous remediation pass did substantial, genuine work: save integrity, the
one-writer checksum discipline, the 46-byte wire format, the VRAM overlap compile
assert, the boss phase fixes, the supercover line-of-sight walk, and the removal
of the 262 KB embedded-video fallback are all real and correctly reasoned.

Three things dominated this review.

**The test suite certified a copy of the game, not the game.** The previous
review marked "the harness now shares a crate with the game" as resolved. In fact
the harness depended on `plattypus-core` for the save format and act metadata, and
mirrored everything else — 76 % of its lines, 211 of 356 assertion sites. Worse,
the mirrors had already drifted: the harness's idea of the Act 1-1 exit tile was
`(19, 3)` while the generator produced `(21, 21)`, and its draw-budget constant
was 1,100 against the renderer's 1,600. **The single most consequential act of
this pass was moving the level generators into the shared crate.** The moment the
harness could call the real ones, two genuine bugs in `has_line_of_sight` fell out
— a corner test that re-checked the tile the walk had already left, and an
overshoot guard that reported every backwards-facing sight line as clear.

**The verification signal was dead.** `cargo fmt --check` failed on the committed
tree, and because the CI job ran `build → fmt → test` in one step, the host test
suite never executed at all. Everything the suite claimed to protect had been
unverified for as long as that gate had been red.

**Several of the previous review's "✅ resolved" verdicts were wrong**, in both
directions — items genuinely fixed were marked partial, and items still broken
were marked done. Each verdict in this document was re-derived from the code at
`9e6a575`, not inherited.

**The licensing question is settled: this is non-commercial homebrew with
copyright retained.** That decision is now stated consistently across `LICENSE`,
the README, the press kit, the plan, the packaging specs and the disc artwork —
see §8.

### At a glance

| | Before | After |
|---|---|---|
| `cargo fmt --check` | **fails** (354 hunks) | clean, all 4 crates |
| Host test suite in CI | **never ran** (fmt aborted the job) | runs, and gates the build |
| `cargo clippy -D warnings` | 1 error + 19 warnings (harness) | clean |
| Test count | hardcoded `48/48` (actually 23) | derived at runtime |
| Level generators testable from host | **no** | yes, via `plattypus-core` |
| Mirrored game logic in the harness | ~1,930 lines | 0 (166 lines deleted) |
| `has_line_of_sight` bugs | 2 (found only by the new tests) | 0 |
| Reachable codenames | 2 of 12 | 12 of 12 (1.3 M-case sweep) |

---

## 2. Principal Developer — architecture

*Focus: module boundaries, state ownership, duplication, invariants.*

**What is genuinely well-structured.** The layering (`main` → `game` →
`renderer` / `entities` / `platypus` / `level` / `save`) is defensible. The
`no_std` discipline is exemplary: `grep -rn "\.unwrap()\|\.expect(\|panic!\|todo!\|unimplemented!\|unreachable!" game/src/`
returns **zero** hits. Memory management is honest — `video::STORAGE` is a fixed
`static mut`, `Game` is a `.bss` symbol rather than a stack allocation, and the
arena does not fragment. There are no magic `unsafe` blocks whose invariants are
only described in prose.

### PD-1 · P1 · The harness mirrored the game instead of testing it — **fixed**

`tools/test_game_logic/src/main.rs` carried hand-written copies of eleven
production functions, among them:

| Mirror | Claims to test |
|---|---|
| `commit_progress` | `game.rs:1160` |
| `generate_act1_1` / `generate_act1_2` / `generate_act3_2` / `generate_vr_speed` | `level.rs` generators |
| `exit_cell_index` | `Level::exit_x / exit_z` |
| `heading_velocity` | `platypus.rs:69-82` |
| `fit_text` / `center_text` | `renderer.rs:91-121` |
| `winding_cross` | `renderer.rs:184-190` |
| `los` | `level.rs:126` |
| `Excavator` (declared *inside* a test) | `entities.rs:361-429` |

Only `SaveData`, `Codename`, `Act` and `CellType` were genuinely shared. Two
mirrors had already drifted away from the code they claimed to describe — see §7
for what that uncovered.

**Why it mattered.** `ALERT_DURATION` could be changed in `entities.rs` and the
alert-expiry test would still pass. Two of the mirrors were not merely stale but
wrong.

**Fix.** The `Level` struct and all sixteen generators moved from
`game/src/level.rs` into `crates/plattypus-core/src/level.rs`, which has no
hardware dependency at all — `game/src/level.rs` was already just
`pub use plattypus_core::level::*;` plus the impl. `game/src/level.rs` is now a
five-line re-export. 166 lines of mirror code were deleted.

### PD-2 · P1 · No CD-read retry exists in the game — **open, video scope**

The previous review recorded the CD retry as resolved. It exists only in the test
mirror. `grep -in "retry|reseek" game/src/video.rs` returns **zero** matches;
`video.rs:495-507` clears `using_cd` on the first read error and never retries.
This is in the video playback path and was deliberately left alone this pass.

### PD-3 · P1 · Duplicate progress counters · PD-4 · P1 · Records painted over — **fixed**

`mission_stats` and `stage_*` are incremented in parallel
(`game.rs:747-748`, `772-773`) and the debriefing was fed campaign totals while
the rank beside it was graded from per-stage figures. And the stage-select
service records — the campaign's only visible progression feedback — were drawn at
`y=202` and then painted over by the footer's opaque background at `y=208`, which
erased the second half of every glyph.

### PD-5 · P2 · Three level-layout queries with zero callers, disagreeing with the inline copies

`is_road_row` / `is_sidewalk_col` / `is_dune_col` (`level.rs:100-112`) are defined
and never called. The renderer hardcodes the same facts at `renderer.rs:730-741`,
and the two disagree: `is_road_row` omits `gz == 6`, and `is_dune_col` says
`gx == 6` where the renderer uses `gx == 1`. **Open** — deliberately sequenced
after the renderer split, so the split does not fossilise the constants.

### PD-6 · P1 · A rejected save was overwritten with no prompt — **fixed**

`Game::new` did `memcard.load_from_slot1().unwrap_or_else(SaveData::new)`, so a
`Corrupt` or `Incompatible` outcome silently became a fresh campaign. The warning
string existed at `renderer.rs:2966-2974` but its only reader was the options
menu — the fourth title item, on a different screen. The file is now routed
through the existing `GameState::ConfirmNewCampaign` prompt.

### PD-7 · P1 · Debriefing counters wrapped instead of saturating — **fixed**

`alert_str[14] = ((alerts / 10) % 10)` and friends wrote two digits from an
unclamped value, so 142 alert phases displayed as `42`. `renderer.rs` already
saturated these correctly in `draw_stage_clear`; the debriefing did not.

### PD-8–PD-10 · P2 · Structural debt — **open**

- ~170 lines of boss damage resolution live in `platypus.rs` rather than with the
  bosses, copy-pasted four times; three inline copies have already drifted.
- The current act has two owners: `EntityManager::act` *and* an `act: Act`
  parameter to `update()`, and the per-act BGM table is written three times.
- The "zero dead-code warnings" safety net does not work: `Level::is_road_row`,
  `SaveData::reset_to_clean_slate` and `Act::from_index` had no callers in
  first-party code, because `pub` on a `pub` type in a **binary** crate is not
  dead code as far as the lint is concerned.

### PD-11 · P2 · `renderer.rs` is 3,405 lines mixing seven concerns — **open**

Text fitting, the VRAM map, camera, row traversal and culling, level-to-visual
mapping, 3D submission, ~2,000 lines of model drawers, the HUD, and seven
full-screen menus. Splitting it is also a *performance* fix — see §3.

---

## 3. Senior Engineer — hardware and performance

*Calibration: `[M]` measured from a built artefact or the disc image; `[D]`
statically derived; `[S]` suspected, needs console confirmation.*

### Measured budget

| Quantity | Value | Source |
|---|---|---|
| `dist/plattypus.exe` | 632,832 B = **30.2 %** of PS1 RAM | `[M]` |
| `video::STORAGE` | **268,800 B = 12.8 %** of RAM; needs 72,512 B | `[D]` |
| RAM in use | ≈ 905 KB of 2,001,152 B link region = **43.2 %** | `[D]` |
| VRAM declared / free | 310,692 B = 29.6 % / 737,884 B = 70.4 % | `[D]` |
| SPU RAM high-water | 118,048 B = **22.5 %** | `[D]` |
| Hot code vs I-cache | **109,676 B against 4 KiB** | `[M]` |
| Movie frames / sectors | 148 frames, 741 sectors, 3.757 video + 1.249 audio | `[M]` |
| 1× drive margin | 9.880 s vs 9.867 s playback = **+0.13 %** | `[D]` |

The RAM and VRAM numbers are healthy. Two things are not: `video::STORAGE` is
3.9× larger than the data it holds (a video-scope finding, deliberately out of
scope this pass), and the hot code does not fit in cache.

### SE-1 · P1 · The quad budget deleted the ground under the player — **fixed**

Rows are walked **far-to-near** so the painter's order stays correct, which means
the budget is consumed from the far edge first — and the tile loop used to `break`
the moment it ran out:

```rust
for gz in (min_gz..max_gz).rev() {
    for gx in row_min_gx..row_max_gx {
        if !self.budget_available() { break; }   // every nearer row skipped
```

Act 2's rapids banks need ≈4,400 quads against a 1,600 budget, so on the game's
most-touted stage **every nearer row was skipped and the ground under the player
disappeared**. The commit that introduced the culling also introduced the budget.
The fallback now draws the bare floor for the rest of the frame when the budget is
gone: walls and detail can be dropped, the floor cannot.

### SE-2 · P1 · The synthesiser could not re-attack — **fixed**

`Adsr::default_tone()` holds sustain level `0xF`, and the SPU has no retrigger.
`key_on` on a voice already at full sustain leaves the envelope alone, so each
"note" kept looping the sample while `set_pitch` stepped between values — one
continuous oscillator, not a melody. All four synth voices now use
`Adsr::percussive()`, which self-fades in the sustain phase.

### SE-3 · P1 · Sustained 2.13× clipping — **fixed**

Three voices at unity main volume summed to **2.13×** of full scale, sustained, in
every level, before any sound effect. `VOICE_BASS` drops from 1/5 to 1/7, bringing
the unconditional trio to 0.48. Main volume was deliberately left at `MAX` —
turning it down would halve every sound effect too, which is a worse trade.

### SE-4 · P1 · No deferred flip — **open**

`renderer.rs:331-347` calls `wait_vblank()` then `fb.swap()`; `framebuf.rs:63-78`
writes `GP1(05h)` plus three raw GP0 words with no `wait_cmd_ready()`. The SDK's
own doc comment at `framebuf.rs:93-104` describes the correct sequence
(`arm_draw_done` → `signal_draw_done` → `queue_gp1_at_vblank`), and
`psx-gpu/lib.rs:171-180` documents that GPUSTAT bit 28 rises *about one large
primitive early* on silicon. Nothing ever checks it. Transcribing the SDK's own
recipe is low-risk but needs an emulator run to confirm.

### SE-5 / SE-18 · P1 · Two DMA block sizes need console verification — **open**

Both `[S]`, both one-line fixes once confirmed, both **invisible on a
permissive emulator** — which is exactly why they are dangerous.

- `video.rs:748-758` requests GPU DMA block size 8. On PS1 silicon the channel-2
  BCR block size is a `2^(n+4)` exponent with a 16-word floor, so this should
  guarantee a PIO fallback after a **103 ms** stall, then latch `vram_dma_ok =
  false` permanently. In video scope; not touched.
- `psx-fmv/mdec.rs:34` uses `DMA_BLOCK_WORDS = 32`; PS1 MDEC wants 16. Also video
  scope.

### SE-6 · P1 · "Frustum culling" is an origin-based AABB reject — **open**

The entire implementation is `renderer.rs:439-453`:

```rust
let dz = (row_z_min - self.cam_z).max(48);
let half_w = (dz * 9 / 10) + 96;
let row_min_gx = ((self.cam_x - half_w) / TILE_SZ).clamp(0, GRID_W as i32) as usize;
```

Five separate problems: there are no planes (the SDK ships
`scene::classify_aabb_clip4`, unused); `dz` is world-Z, not view depth, and the
camera has a 47.8° pitch and ≈232 units of height offset; tiles are culled by
**origin**, so up to one tile of floor is missing at each screen edge;
`ent_max_dist` is computed from the row's *near* edge but applied across the whole
64-unit row; and it is applied to tiles only — Platty, collectibles and every boss
are not culled at all. Derived from screenshots, the horizontal test admits the
entire 24-tile row; the row-range test is doing all the work.

### SE-7 · P2 · Vision cones z-fought with the floor — **fixed**

Cones were issued **before** the tile traversal with every vertex one world unit
above a floor drawn at zero. The PS1 has no depth buffer and no polygon offset, so
which of two coplanar primitives won was a sub-pixel rounding race — and "a guard
has spotted you" was flickering frame to frame. The decal pass moved to after the
row loop, and the vertices were raised to `CONE_DECAL_Y = 4`.

### SE-14 · P2 · Verify GP0(E1) bit 10 — **open, highest leverage per unit effort**

`psx-hw/src/gpu.rs:283-298` places `draw_to_display` at **bit 10**, which on PS1
silicon is a draw-to-VRAM bit, and passes `true` from four call sites reached
every frame. If confirmed, the whole scene would render into framebuffer A,
ignoring the back-buffer draw offset, from the first HUD text onward. One register
read decides it; the fix is `(draw_to_display as u32) << 0`. Invisible on the
emulator that produced the reference screenshots.

### SE-13 · P2 · 205 unbudgeted draw sites — **open**

`draw_quad_3d` is an associated `fn` with no `&self`, so it charges no budget at
all; the vision-cone and searchlight passes iterate every sentry and searchlight
with no distance test; and the entity loops have no `budget_available()` check, so
only tiles were ever gated. The per-box checks use `continue`, producing
**half-drawn models** (Platty with a body and no bib) when the budget runs out.

### SE-16 / SE-17 · P3 · Dead title-music path; no dithering — **open**

The 23.6 MB CD-DA track is the only recorded music; `Music/title_music.adpcm` is a
127 ms loop with no call site, so it is 3.2 KB of dead ROM and SPU rather than an
audible defect. Dithering is disabled everywhere on a 15-bit framebuffer with
32 levels per channel, which is why the dark city and river palettes band.

### SE-19 · P2 · The CPU profile in the tree is not this game — **open**

`scratch/cpu.csv` and `cpu2.csv` hold 1,200 rows in which `profiled_cpu_cycles` is
**571,237 on every row** — exactly one 60 Hz display period — with `tape_frame`
pinned at 0. It is a constant-workload synthetic trace, and `rg` finds **zero**
references to it from the Makefile, tools, game source or bench. **There is no
per-frame profile of Plattypus at all**, which is why every performance figure
above is `[D]`. The instrument already exists: `log_performance_telemetry` prints
`vb=`, `quads=`, `stutters=` every 60 frames. One minute of Act 2 on hardware would
convert SE-1 and SE-20 from derived to measured.

---

## 4. Product Designer — UX

*The mechanical remediation of the previous pass is real: tripwires hurt, guards
respect line of sight, motion is heading-scaled, the boss has phases, PAL actually
reprograms the GPU. But the rank fix and the objective text were both broken in a
new way.*

### UX-17 · P0 · The final boss cannot hurt you — **partially fixed**

Every `take_damage` site is gated on an act or entity that Act 4-3 never populates.
`load_act4_3` (`entities.rs:1342-1362`) spawns only the boss and two rations; no
sentry, no tripwire, no vehicle, and `beach_crabs` are all inactive. The
excavator's claw sweep and slime mortar (`entities.rs:2136-2156`) have **no player
hit test at all** — they are animation plus particles.

*Not fixed this pass.* This needs a hit test with telegraphs and fair windows; it
is a design decision, not a mechanical fix, and it should not be landed blind
alongside everything else.

### UX-18 · P0 · Ten of twelve codenames were unreachable — **fixed**

`Codename::evaluate` was called in exactly one place, on the final mission. Act
4-3 has no sentries, no tripwires and no damage source, so the evaluator was always
called with `(0 alerts, 0 damage, t, 0 takedowns)` — which is `BigPlatypus`,
Rank S, on every playthrough. Nine of the twelve names and the whole A/B/C/D
ladder could never be earned, and the Stealth Camo unlock was automatic.

Ranks are now graded on **every** stage and the best is carried as the campaign
record, alongside the final mission's own rank. A new exhaustive sweep evaluates
all four axes (1.3 M cases) and asserts that all twelve ranks are reachable, that
time can only ever lower a rank, and that `rank()`/`from_index` round-trip.

### UX-19 · P0 · Two overlapping strings on the first button press — **fixed**

Screenshot-proven. `scratch/screens_run2/tick-000300.png` reads:

```
DPAD: SELECT MISSION PROGRESS SAVED ... CONFIRM
```

The title footer was drawn at `y=216` and the save-status OSD box at `y=214`, and
starting a new campaign saves on the way in — so the first thing a new player does
produced overprinted mush on the one screen they are reading. The footer moved to
`y=204`, between the menu box and the OSD.

### UX-22 · P1 · The game never states an objective — **fixed**

`rg "\.subtitle\(\)" game/src/ crates/` returned exactly one hit: the definition.
Sixteen mission briefings existed and had never reached a pixel. The subtitle is
now drawn along the bottom of the HUD as dim reference text.

### UX-24 · P1 · The player vanished for 1.5 s of every stage — **fixed**

`reset_position` set `invuln_timer = 90`, and the renderer skipped the entire
Platty model — including the cardboard box — for half of that. Exactly the window
in which the player reads a new room. Reduced to 30 frames.

### UX-23 · P1 · The tutorial taught a dead button — **fixed**

`codec.rs:124` told a first-time player to crawl "with CIRCLE or DOWN". Only
`platypus.rs:358` binds CIRCLE; `button::DOWN` appears once, as a movement
direction. A player who pressed DOWN hard-blocked on the first vent. Corrected to
"CIRCLE".

### UX-25 · P1 · The sonar could disable a stage permanently — **fixed**

The pulse stunned anything within 150 units on both axes, **through walls**, for
300 frames — longer than its own 180-frame recharge. One held button removed the
entire detection layer of a stage, silently. It now respects line of sight, reaches
90 units, and stuns for 150 frames.

### UX-26 · P1 · "Full 360° analog speed" delivered three speeds — **fixed**

`speed = (max_speed * mag) / 127` with `max_speed = 4` means post-deadzone
magnitudes 65–95 all yield **speed 2** — identical to a stalk — while the player is
classified `Running` and emits full footstep noise at half speed. Speed 4 required
the stick at absolute maximum. The top two speeds are now spread across the
committed half of the travel, with named thresholds.

### UX-29 · P2 · The attract demo could never start — **fixed**

```rust
CDDA_WAS_PLAYING && !Self::is_cdda_playing()
```

`CDDA_WAS_PLAYING` is only ever set *inside* `is_cdda_playing()`, and
`reset_cdda_tracking()` clears it as the title screen starts its track. The `&&`
short-circuits, the poll never happens, the flag never gets set, and all 140 lines
of `AttractDemo` were unreachable. The poll now runs first.

### UX-28 · P2 · The options footer overprinted its own counter — **fixed**

`"CIRCLE: RETURN TO TITLE SCREEN"` reaches `x=266`; the stutter counter started at
`x=228`. Every option row read `CIRCLE: RETURN TO TITLE SCRSTUT:000`.

### Screenshot observations

Read at 320×240 native from `scratch/screens_run{1,2,3,4}/`. `crop_bottom_right.png`
pixel-matches the bottom-right quadrant of `screens_run3/tick-001100.png` at 1:1.

**Strong.** The HUD is the best thing on screen — `LIFE` pips, stage label, score,
`YAB:x00`, all inside opaque panels, fully legible with zero bleed. The CODEC is
excellent: framed panels, an animated waveform bar, a correctly sized speaker tag
derived from `font.text_width`, and a typewriter mid-reveal. The electro-sonar
overlay is the best screen-reading work in the codebase. The excavator boss gauge
names the mechanic, telegraphs the window, and withholds what is not actionable.

**Problems visible in the frames.** No drone blips on the radar despite six drones
in Act 1-2. Platty is a ~15×14 px blob while nearby crates are ~40×30 px, with only
a shadow as an identifier and no camera control of any kind. The level reads as a
rectangular plateau with a hard cut edge and pure black beyond — nothing is drawn
outside the grid. Wall texture aliases severely at grazing angles. Platty's own
CODEC portrait is a 76×91 px grey checkerboard that reads as a missing asset. No
missing textures, no z-fighting, no wrong-palette evidence.

### Deliberately well-designed — do not touch

1. The electro-sonar recon overlay.
2. The excavator boss HUD.
3. The text-fitting policy — one measure-once-clamp-once rule shared by the
   renderer *and* the CODEC, with the decision documented: tighten the inter-glyph
   gap by −1 px rather than truncate, because the atlas is ASCII-only.
4. `has_line_of_sight`'s supercover walk and its stated bias — *"Erring toward
   'cannot see' is deliberate."*
5. The acoustic-surface model coupled to contextual prompts — the player is taught
   each verb at the moment it becomes useful. This should be the template for the
   rest of the HUD.

### Still open (designer)

| ID | Finding |
|---|---|
| UX-17 | Final boss hit test — **needs a design decision** |
| UX-30 | Four VR sims render as a sunny beach (chapter 5 falls through every chapter match) and award `[CLEARED]` for walking to the exit, ignoring their own stated objectives |
| UX-27 | The Wireframe unlock leaves every wall, container and floor textured; `rg "self.wireframe"` has one hit, inside the model path |
| UX-33 | Camera has no control, no zoom, and sits low enough that a near wall smears across a third of the screen |
| UX-35 | `MANUAL.md:92-101` documents `HEALTH`, `RATIONS`, `LETTER`, `CQC SPUR ARMED` and `SURFACE:` — none of which exist; no drone blips, noise ring or jamming state on the radar; no surface readout despite the value existing at `platypus.rs:243-259` |
| UX-37 | No difficulty option (`rg -ni difficulty game/src/ crates/` → 0 hits), no brightness/gamma, screen offset is vertical-only |
| UX-43 | `DualShockController::deadzone` is a dead field; the comment at `platypus.rs:52-55` claims the right stick aims the sonar and nothing reads it |
| UX-26/32/34 | Analog curve ✅ fixed. Stage-clear card mixes campaign and per-stage figures; `game.rs` never adopted the text fitter, so six overlay strings overflow (worst: Game Over's controls, 34 px off-screen) |

---

## 5. Marketing — positioning and distribution

*This section contains the only findings with genuine legal exposure. None of them
are code fixes; all of them are one-line text edits, which is exactly why they
persisted.*

### The defects

| Finding | Issue |
|---|---|
| MK-10 · P0 | `DISC_SURFACE_SPEC.md:50` asserted **"Licensed by Sony Computer Entertainment Inc."** — a licence that does not exist, contradicting `LICENSE:29-31` |
| MK-11 · P0 | Hand-drawn **ESRB "EVERYONE 10+"** and **PEGI "7+"** badges. Neither body has rated this game. `JEWEL_CASE_SPEC.md:57` also specified **ELSPA**, an organisation discontinued in 2003 |
| MK-12 · P0 | A drawn **Sony "Seal of Quality"** on the NTSC cover — a licensed mark |
| MK-13 · P0 | Two barcodes on GS1 prefixes belonging to **other companies** (`711719`, `017783`) |
| MK-14 · P0 | **Five product codes** for one SKU across ten surfaces; `BASLUS` (a US prefix) on the disc face contradicts `SLUS-00001` on its own region's insert |
| MK-15 · P0 | The only press screenshot was described as a *"320x240 native PS1 frame buffer capture"*. It is **1275×769 key art** |
| MK-16 · P0 | The back insert's "3 Full-Color Game Screenshots" are **three circles and two polygons** |
| MK-21 · P0 | `ffprobe` reports `TAG:encoder=Google` on both cinematics; Google's free-tier AI output is **non-commercial**. *(Resolved once the project was confirmed non-commercial — see §8.)* |
| MK-22 · P0 | Disc face said **"All rights reserved. Unauthorized copying prohibited"** — incompatible with the GPL the project ships under |
| MK-23 · P0 | **32 MB of media with no recorded provenance**, and no `ASSETS.md`. The SDK vendors `PROVENANCE.md` and `report.json` for exactly this; the game did not. *(Subsequently resolved: all of it is Gemini-generated — see §8.)* |
| MK-24 · P0 | `LICENSE` says "non-commercial homebrew"; `README`, `PRESS_KIT` and `PLAN` say "commercial retail packaging" |

All of the above are fixed except MK-21 and MK-24, which are decisions rather
than edits — see §8.

### Claim-by-claim verification

Every factual claim in `README.md` was checked against the code and the build.

| Claim | Verdict |
|---|---|
| 4 chapters, 12 acts, 4 bosses | ✅ `save.rs:173`, `level.rs:144-149` |
| 4 VR training missions | ✅ |
| Act names, all 12 | ✅ match `level.rs:73-88` one-for-one |
| GTE transforms, Gouraud, affine texturing | ✅ |
| 320×240 at 15 fps via MDEC | ✅ with a caveat — 1× drive speed is not optional |
| **"interleaved 37.8 kHz CD-XA ADPCM streaming to the SPU CD mixer"** | ❌ **false** — `play_intro_audio()` and `play_outro_audio()` are empty bodies; the player falls back to SPU samples |
| Red Book CD-DA on Track 2 | ✅ it is the 2:14 menu loop, and the only recorded music in the game |
| **"46 automated host test suites"** | ❌ **23 run**; the banner hardcoded `48` |
| **"custom animated 16x16 3-frame BIOS save icon"** | ❌ **one static frame**; the PS1 BIOS format cannot animate |
| Circle = "Paddle Forward" | ❌ CIRCLE is never read in water |
| Start = "Skip Transmission" in the CODEC | ❌ START is read only in `game.rs:179` and `video.rs:592` |
| `git clone --recurse-submodules <repository-url>` | ❌ literal placeholder |
| CI badge | ❌ relative URL — cannot resolve |
| DualShock analog & rumble | ✅ real, 22 trigger sites, two-motor `0x4D` |
| Tripwires, box, crawl, sonar, CQC, radar | ✅ all genuinely implemented |

Four claims were false, and three more were unverifiable fabrications in the
manual. All corrected this pass except the two video-playback claims.

### Honest assessment of the marketing surface

Before this pass: one image, described incorrectly; four SVG mockups; no logo; no
trailer; no GIF; no store page; no demo; no announcement plan; no changelog;
**1/10** for discoverability.

The legal and claim defects are fixed and the repo now has `ASSETS.md`,
`CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `.editorconfig`, an absolute CI
badge, a working clone URL, accurate submodule instructions, and a documentation
table that surfaces `docs/perf/why-6fps.md` — the best story the project has, and
previously invisible to anyone who landed on the repo.

Still absent, and needed before launch: a logo or wordmark, 8–12 real 320×240
captures (~3,700 already sit untracked in `scratch/screens_run*/`), a 30 s trailer,
an itch.io page, a `v1.0.0` tag, and real screenshots in the back insert.

---

## 6. Testing — strategy and automation

### The suite was not running

CI ran `build → fmt → run tests` in a single job. `cargo fmt --check` failed on
the committed tree, so **the host test suite never executed**. Everything the
suite claimed to protect had been unverified for as long as the gate was red.

### The suite certified a copy

The previous review recorded "zero dependency on game code" as a *strength*. It was
accurate and it was the problem. Measured: **356 assertion sites, 0 `#[test]`,
0 references to `game/`, and ≈1,930 of 2,545 lines (76 %) reimplementing game
logic.** One assertion was a tautology — `assert!(a && !x || !a && !x)` reduces to
`assert!(true)` — and it was the only line clippy rejected outright.

Harness constants had drifted from production, in suites the remediation table
marked green:

| Constant | Production | Harness | Consequence |
|---|---|---|---|
| `QUAD_BUDGET` | **1,600** | 1,100 | the draw-budget test asserted against a budget the game does not have |
| far-Z cull bound | `cam_z + 768` | `cam_z + 800` | passed for the wrong reason |
| Act 1-1 exit tile | **`(21, 21)`** | `(19, 3)` | the exit-reachability test checked a cell the generator never produces |
| analog speed curve | 5 regimes | 4 | the water branch was never swept |

### What moving the generators into core uncovered

The moment the suite could call the **real** `Level::new`, three new tests ran
against real geometry and found real bugs that no mirror could ever have caught:

1. **Act 1-3's exit is inside a wall.** `generate_act1_3` sets the exit to cell
   `(12, 1)` and then explicitly paints cells `(11..13, 1..2)` as `Wall` —
   including its own exit. Correct by design (a boss arena is sealed; you leave by
   killing the boss), and the test now distinguishes boss acts.
2. **`has_line_of_sight` corner-crossing test was a no-op.** After stepping
   diagonally it tested `(gx - step_x, gz - step_z)` — the tile the walk had just
   come *from*, already tested at the top of the loop. The two tiles the line
   actually straddles were never checked, so a diagonal squeeze between two walls
   was shootable through. That is the exact case the function's own comment says it
   handles.
3. **`has_line_of_sight` overshoot guard was direction-dependent.** `gx > end_gx`
   fires on the *first step* of any backwards walk, returning "visible" immediately.
   Visibility is symmetric; the function was not. Fixed with a direction-aware
   guard, plus canonicalisation of the walk direction so both call orders run the
   identical walk.

A ~254,000-pair symmetry sweep now runs on every test invocation.

### Current state

| | Value |
|---|---|
| Suites | **23**, counted at runtime |
| `#[test]` functions | 0 — still a hand-rolled binary |
| Assertions | ~360, including exhaustive sweeps |
| Coverage of the *real* level generators | 16 of 16 acts |
| Exhaustive sweeps | 352 single-bit checksum flips; 1.3 M codename evaluations; 254 k LOS pairs |
| `clippy -D warnings` | clean on both host crates |
| `cargo fmt --check` | clean on all four crates |

### Still open (testing)

| ID | Finding |
|---|---|
| QA-3 | **No `#[test]` anywhere in first-party code.** `cargo test` on the core crate runs 0 tests. A failing assertion aborts the whole binary, so later blocks never run and you learn nothing about what else would fail. Converting the ~145 production-touching assertions to `#[test]` is the single biggest remaining test win. |
| QA-17 | `host_bench.py` writes JSON and asserts nothing; nothing runs it in CI. The committed `docs/perf/baseline-intro-video.json` records a **failing** state (`fps: 6.0` against a 15.00 target) and nobody fails a build over it. `Makefile:83` passes `-s INT` to `timeout`, which truncates the transcript — that is why it reads 6 fps instead of erroring. |
| QA-18 | 21 MB of captured frames in `scratch/` are untracked and **no script in the repository produced them**. A deterministic replay harness (scripted input tape + fixed tick) plus golden-frame comparison would be high value; the simulation is already fully deterministic because there is no RNG. |
| QA-20 | No `timeout-minutes`, no matrix, no caching, no SHA-pinned actions. `.git` is **102 MB** with no `.gitattributes` and no LFS. |
| QA-24 | Twelve public `plattypus-core` functions have no test at all, including `service_records_from` — the formatter behind the stage-select records — and `LoadOutcome::label`, whose six strings are the player's only card-status feedback. |
| — | `.git` grew from 66 MB to 102 MB this cycle, largely a **7.6 MB prebuilt WASM blob** under `web/player/` plus a 1.8 MB `THIRD-PARTY-NOTICES.txt`. There is no build provenance for the bundle: no toolchain pin, no reproducible build, nothing recording what produced it. |

---

## 7. What was fixed

Every item below was implemented and verified in this pass.

### Build and test infrastructure

| Change | Detail |
|---|---|
| `cargo fmt` gate unblocked | 354 hunks formatted; all four crates clean. **This alone restored the CI test signal.** |
| clippy clean | Fixed the 1 error (`overly_complex_bool_expr`) and all 19 warnings; both host crates now pass `-D warnings` |
| Tautology removed | `assert!(a && !x \|\| !a && !x)` → real assertions of the production predicate in both directions |
| Compile-time invariants | 5 runtime asserts on `const` values → `const _: () = assert!(...)`, matching the idiom already used in `renderer.rs` and `save.rs` |
| Test count derived | `48/48` hardcoded → counted at runtime. The suite prints **23** |
| CI restructured | 3 jobs: cheap `host` checks first, then `game`, then `disc`. A red fmt no longer hides a green test run. Added clippy, `--offline`, `--locked`, timeouts, `permissions`, cargo caching, a lockfile-agreement check, and an executable-size gate against the 2 MB RAM budget |
| `make iso` in CI | `deploy-pages.yml` publishes the ISO; nothing in CI built it |
| Cue validated | CI now checks that the cue names a file that exists |

### The structural change

| Change | Detail |
|---|---|
| Level generators moved to `plattypus-core` | `game/src/level.rs` (1,058 lines) → `crates/plattypus-core/src/level.rs`. `game/src/level.rs` is now a 5-line re-export. The core crate has no hardware dependency, so the suite now exercises real geometry |
| 166 lines of mirror code deleted | `generate_act`, `generate_act1_1`, `generate_act1_2`, `generate_act3_2`, `generate_vr_speed`, `blank_grid`, `set_cell`, `exit_cell_index`, `type Grid` |
| New real-geometry tests | Act 1-2 tripwires from the real generator; per-act exit validity, spawn validity, closed perimeter and **flood-fill exit reachability**; a 254 k-pair LOS symmetry sweep |
| Codename sweep | 1.3 M-case 4-D evaluation asserting all 12 ranks reachable and time-monotonicity; `rank()`/`from_index` round-trip |

### Real bugs found by the new tests

1. `has_line_of_sight` corner-crossing test re-tested the already-visited tile; the two straddled tiles were never checked.
2. `has_line_of_sight` overshoot guard fired on the first step of every backwards walk, reporting those sight lines as clear.
3. `has_line_of_sight` was asymmetric — the Bresenham tie-break is direction-dependent. Now canonicalised.

### Gameplay and feel

| Fix | Detail |
|---|---|
| **Codename reachability** | Graded per stage, best carried as the campaign record. 10 dead ranks restored; Rank S is earned |
| **Title screen text collision** | Footer moved `y=216 → 204`, clear of the save OSD. Screenshot-proven mush |
| **Stage-select records** | Given their own two-line band; row pitch tightened 27→26 px to make room. Columns no longer overlap or run off-screen |
| **Final boss damage** | **Not fixed** — needs a design decision |
| **Objective text** | `Act::subtitle()` wired into the HUD; 16 briefings now reach a pixel |
| **Player blink** | Stage-load invulnerability 90 → 30 frames |
| **Analog speed** | Top two speeds spread across the committed half of stick travel; named thresholds |
| **Sonar stun** | Line-of-sight gated, radius 150 → 90, stun 300 → 150 frames (below the 180-frame recharge) |
| **Attract demo** | `is_cdda_playing()` polled before the flag test; 140 lines of `AttractDemo` are reachable |
| **Tutorial text** | "CIRCLE or DOWN" → "CIRCLE" |
| **Corrupt save** | Routes through the existing `ConfirmNewCampaign` prompt instead of silently overwriting |
| **Debriefing counters** | New saturating `write_fixed` helper; 142 alerts no longer displays as `42` |
| **Options footer** | Counter moved to `x=272`; no longer overprints the control hint |
| **Vision cones** | Decal pass moved after the row loop; vertices raised to `CONE_DECAL_Y = 4`. Stops "you have been spotted" flickering |
| **Quad budget** | Floor preserved when the budget is exhausted instead of skipping every nearer row |
| **Frame counter** | `u8 → u32`; the title cursor pulse no longer stutters every 256 frames and telemetry stops drifting |
| **Music envelope** | `Adsr::percussive()` on all four synth voices — the sequencer is now audible as notes |
| **Music clipping** | `VOICE_BASS` 1/5 → 1/7; sustained sum 2.13× → 0.48×, main volume untouched |

### Documentation, packaging and distribution

| Fix | Detail |
|---|---|
| Sony licence line removed | `DISC_SURFACE_SPEC.md` |
| Forged ratings removed | ESRB and PEGI badges replaced with dashed `NOT YET RATED` placeholders; the ELSPA instruction deleted |
| Sony Seal removed | Replaced with a publisher lockup using "PlayStation" nominatively |
| Barcodes neutralised | Both GS1 numbers → `0 000000 000000` with an explicit DO-NOT-PRINT note |
| Product codes unified | Two SKUs, used consistently; the single-disc dual-region claim corrected |
| Disc face | "All rights reserved" replaced with a GPL-accurate rights line |
| Four false README claims | CD-XA, 3-frame icon, 46 test suites, "Commercial Retail Packaging" |
| Three false press-kit claims | Same, plus the screenshot misdescription |
| CI badge + clone URL | Absolute badge; real URL; stale submodule instructions replaced (the SDK is vendored, not a submodule) |
| `web/roms/` gitignored | 29 MB of game data could be committed by accident |
| Documentation table | `docs/perf/why-6fps.md` — the project's best story — is now discoverable from the README |
| New files | `ASSETS.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `.editorconfig` |
| **Licensing position made explicit** | `LICENSE` now states copyright retained, not public domain, builds free and not for sale, and that the GPL imposes no commercial restriction of its own. `PLAN.md`, `TODO.md`, `JEWEL_CASE_SPEC.md` and the PAL artwork reframed from commercial retail to personal print specs |
| **SVG files made valid** | All four packaging SVGs contained bare `&` and were not well-formed XML — `jewel_case_back_ntsc.svg` fails to parse, so Inkscape/Illustrator would reject it. Pre-existing; 15 ampersands escaped |

---

## 8. Still open

Ordered by what is likely to hurt most.

### Needs a human decision

| ID | Decision |
|---|---|
| **UX-17** | Give the final boss a hit test. The campaign climax cannot currently fail. This is a design call about telegraphs and fair windows, not a mechanical fix |

### Resolved by ruling

**MK-24 — settled: this is non-commercial homebrew with copyright retained.**
The ambiguity was between `LICENSE:29-31` ("non-commercial homebrew") and the
"commercial retail packaging" framing in `README.md`, `PRESS_KIT.md`, `PLAN.md`
and the packaging specs. The project is non-commercial homebrew, so every
commercial framing was removed and the licensing position was made explicit:

- `LICENSE` now states plainly that copyright is retained and **not** dedicated
  to the public domain, that official builds are free and **not for sale**, and
  that no commercial use is authorised or endorsed. It also records that the GPL
  itself imposes no commercial restriction — "non-commercial" is a statement of the
  author's intent, not a limit the licence imposes on others — and that anyone
  commercialising it must clear `ASSETS.md` themselves.
- `PLAN.md`, `TODO.md`, `JEWEL_CASE_SPEC.md` and the PAL artwork no longer
  describe a retail programme; the specs are framed as a personal print guide.
- `README.md` and `PRESS_KIT.md` already said "not for sale"; verified.
- The disc face carries the non-commercial, not-affiliated line.
- `MANUAL.md` and `web/index.html` contained no commercial claims.

**MK-21 — resolved.** Both cinematics are Gemini-generated, and Google's free-tier
AI-output terms are non-commercial — which is exactly how this project is
distributed. No re-generation needed.

**MK-23 — substantially resolved.** All the music is Gemini-generated as well
(`Menu-King_of_the_Yarra.mp3` → Track 02, `Credits-Below_the_Reeds.mp3` →
Track 03, both 44.1 kHz stereo). So there is **no third-party rights-holder to
clear anywhere in the media set** — the earlier "32 MB of unlicensed assets" was
really "32 MB of AI output whose terms were never written down". `ASSETS.md` now
records the full provenance, including which `.mp4` visualisers are unused.

One item remains open, and it is a documentation task rather than a legal one:
**the plan/tier and generation date are not recorded anywhere**, and Google's AI
terms have changed repeatedly. Retaining a copy of the terms in force at
generation time is what makes this auditable. `ASSETS.md` records what to capture.

A caveat worth understanding rather than fixing: purely AI-generated output with
no meaningful human authorship is generally **not eligible for copyright
protection**, which affects the *author's* ability to assert an exclusive right in
the music, not the right to distribute it. `LICENSE` retains copyright in the code
and artwork; the credits roll should not claim authorship of the tracks.

**This ruling also retires the barcode objection.** GS1 prefixes are only issued for
products being sold, so the placeholder barcode position is the correct outcome for
non-commercial homebrew rather than a compromise.

One unrelated defect was found and fixed while checking the artwork: **all four
packaging SVGs contained bare `&` characters and were therefore not
well-formed XML** — `jewel_case_back_ntsc.svg` fails to parse at line 104, which
would stop Inkscape or Illustrator opening it. This predates the current work
(verified against `HEAD`); 15 ampersands are now escaped and all four files
validate.

### Needs hardware

| ID | Finding |
|---|---|
| **SE-14** | Verify GP0(E1) bit 10. One register read decides whether the game renders into the correct framebuffer on a console. Highest leverage per unit effort in this document |
| **SE-5 / SE-18** | Verify the two DMA block sizes. Both invisible on a permissive emulator |
| **SE-4** | Adopt the SDK's deferred flip. The SDK's own doc comment describes the correct sequence |
| **SE-19** | Capture one minute of telemetry on hardware. Every performance figure in §3 is derived, not measured |

### Should be done next

| ID | Finding |
|---|---|
| **SE-1 (residual)** | The floor is now preserved, but a 1,600 budget against a 4,400 need still means Act 2 loses walls. Split `TILE_BUDGET` from `ACTOR_BUDGET` |
| **SE-6** | Implement real frustum culling with correct view-depth math, using the SDK's `classify_aabb_clip4`. Cull by bounds, not origin; apply it to entities |
| **SE-13** | Bring the ~205 unbudgeted draw sites under budget; make over-budget boxes all-or-nothing rather than half-drawn |
| **SE-20 / PD-11** | Split `draw_3d_scene`. 10.3 KiB of instructions against a 4 KiB I-cache, entered once per frame |
| **QA-3** | Convert the suite to `#[test]` so failures name themselves |
| **QA-17** | Fix the `-s INT` in `host_bench.py`, then gate frame rate and stutter count in CI |
| **UX-22 (follow-on)** | The objective line is permanent. Consider a timed banner instead |
| **PD-5** | Wire up the dead level-layout queries, or delete them — before the renderer split |
| **PD-8 / PD-9** | Unify the boss damage API; remove the duplicate act ownership and the triplicated BGM table |
| **UX-30** | The VR sims need a dark palette and their objectives enforced — they are the first thing a new player tries |
| **QA-18** | Build the deterministic replay + golden-frame harness |
| — | `web/player/` commits a 7.6 MB prebuilt WASM blob with no build provenance; `.git` is 102 MB |

---

## 9. Appendix: verification performed

Commands run against `9e6a575` with a clean tree:

```
cargo build --release            (from game/ — the cwd-scoped .cargo/config.toml
                                  means the root invocation fails, which is QA-7)
cargo fmt --all -- --check       (all four crates: clean)
cargo clippy --all-targets -- -D warnings   (both host crates: clean)
cargo run --manifest-path tools/test_game_logic/Cargo.toml   (23/23 pass)
stat -c%s game/target/mipsel-sony-psx/release/plattypus.exe   (632,832 B = 30.2 % RAM)
```

Screenshots at `scratch/screens_run{1,2,3,4}/` were read at native 320×240, and
`crop_bottom_right.png` was pixel-matched against every frame's quadrants to
confirm it is a 1:1 crop rather than a magnification.

**Not verified:** anything requiring console or emulator execution. Every
performance figure in §3 is statically derived and labelled `[D]`; every DMA and
register finding is `[S]`. `make disc`, `make iso`, `make web` and the DuckStation
bench were not run.

**Deliberately out of scope this pass:** video and MDEC playback, per the
instruction. That excludes PD-2 (CD retry), SE-5, SE-8, SE-10 and SE-18 — all
real findings, all deferred, none downgraded.