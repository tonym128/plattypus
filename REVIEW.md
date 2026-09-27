# Plattypus — Comprehensive Multi-Perspective Game Review

A thorough review of the complete 10,589-line codebase across 15 source files, 12 campaign stages, 4 VR training missions, 4 boss encounters, and full PS1 hardware integration.

---

## 🏗️ Principal Developer — Architecture & Technical Debt

### Bugs & Logic Issues

| # | Severity | File | Issue | Recommendation |
|---|----------|------|-------|----------------|
| **PD-1** | 🔴 **Critical** | [game.rs:679](file:///home/tonym/Projects/plattypus-psoxide/game/src/game.rs#L679) | **Continue after campaign completion loads VR stage.** After beating Act 4-3 (index 11), `unlocked_act` is set to `12`. `Act::from_u8(12)` maps to `VrSneaking`. Selecting "CONTINUE" on the title screen loads VR-01 as if it were a campaign stage with a story CODEC briefing. | Clamp `unlocked_act` to `11` after Act 4-3, or treat `unlocked_act >= 12` as "campaign complete" and either replay Act 4-3 or show a "Campaign Complete — New Game+" prompt. |
| **PD-2** | 🟡 **Medium** | [platypus.rs:225](file:///home/tonym/Projects/plattypus-psoxide/game/src/platypus.rs#L225) | **Oxygen depletes too fast.** Air drains `1` per 2 frames = ~3.33 seconds total underwater. Combined with `1 damage per frame` at air=0, this is punishingly lethal. For Act 4-2 (Pier Shark Trench) which requires sustained diving, this may be too tight for casual players. | Consider draining every 3-4 frames (~5-6.7 seconds total), or start with `air = 150`. Keep the current rate as a "Hard Mode" option if desired. |
| **PD-3** | 🟡 **Medium** | [save.rs:95](file:///home/tonym/Projects/plattypus-psoxide/game/src/save.rs#L95) | **Speedrun codename "Sly Possum" threshold (< 450s / 7:30) may be unrealistic.** 12 stages × ~37.5 seconds each average is extremely aggressive for a stealth game. Unless `time_s` only counts gameplay frames (not CODEC/cutscenes), this rank may be effectively unachievable. | Verify whether CODEC dialogue and cutscene time is excluded from `play_time_frames`. If not, either exclude it or raise the threshold to ~900s (15 minutes). |
| **PD-4** | 🟢 **Low** | [game.rs:407](file:///home/tonym/Projects/plattypus-psoxide/game/src/game.rs#L407) | **D-pad UP = South (+Z), DOWN = North (-Z).** This is intentional and documented, but counter-intuitive. In most stages the player starts at Z=21 and exits at Z=2, meaning pressing DOWN moves "forward" toward the objective. | Consider swapping so UP moves toward the exit (North / -Z) to match player expectation, or add a brief on-screen compass during the first play. |
| **PD-5** | 🟢 **Low** | [platypus.rs:103](file:///home/tonym/Projects/plattypus-psoxide/game/src/platypus.rs#L103) | **`reset_position` sets angle to 128 (North) but Act 1-1 exit is South.** In Act 1-1, the player spawns at Z=2 and must reach Z=21 (South). Starting facing North means the player initially faces away from the objective. | Set spawn facing angle based on whether the exit is North or South of the spawn point. |

### Code Quality Observations

| # | Area | Observation |
|---|------|-------------|
| **PD-6** | Magic Numbers | Over 120 hardcoded numeric constants scattered through gameplay code (collision radii, stun durations, speeds, scores). These should be extracted into named `const` declarations at the top of each module for maintainability and tuning. |
| **PD-7** | HUD in Renderer | All HUD drawing (~500 lines) lives in [renderer.rs](file:///home/tonym/Projects/plattypus-psoxide/game/src/renderer.rs) rather than a dedicated `hud.rs`. This file is 2,377 lines — the largest in the project. Extracting HUD into its own module would improve readability. |
| **PD-8** | No TODO/FIXME | Zero TODO or FIXME comments remain — all development checklist items in `TODO.md` are marked complete. This is excellent. |
| **PD-9** | Test Coverage | The 4 test suites cover save data, codename ranking, act progression, and collision mechanics. Missing coverage: boss state machines, collectible pickup logic, oxygen depletion/recovery, electro-sonar stun radius, and river current physics. |
| **PD-10** | Fixed-Point | `fixed.rs` (87 lines) implements 16.16 fixed-point math but is never imported or used by any game module. It should either be used for deterministic physics or removed as dead code. |

---

## ⚙️ Senior Engineer — Performance, Stability & Hardware

### Performance

| # | Area | Issue | Recommendation |
|---|------|-------|----------------|
| **SE-1** | GPU Fill Rate | Ground contact shadows (new) add a flat quad draw call per Crate, Container, and AirDuct tile. In worst case (a level densely packed with crates), this could add ~50+ extra `GP0(0x28)` quad submissions per frame. | Profile on real hardware or accurate emulator (Mednafen). The PS1 GPU rasterizes ~360K flat-shaded pixels/frame at 30fps; these small shadow quads are likely <5% overhead, but verify. |
| **SE-2** | Row Sorting | The painter's algorithm row-based depth sort in [renderer.rs:160-166](file:///home/tonym/Projects/plattypus-psoxide/game/src/renderer.rs#L160-L166) iterates entities per row per type (sentries, drones, searchlights, vehicles, crabs, collectibles, particles). With `MAX_SENTRIES=6, MAX_DRONES=3, MAX_COLLECTIBLES=16, MAX_PARTICLES=24`, each row scans ~67 entities. Over ~10 visible rows, that's ~670 entity checks per frame. | This is acceptable for PS1 R3000 at 33MHz. No change needed unless profiling shows a bottleneck. |
| **SE-3** | MDEC Fallback | [video.rs](file:///home/tonym/Projects/plattypus-psoxide/game/src/video.rs) has a PIO fallback if DMA Channel 1 stalls (10,000 spin limit). This is defensive but could silently drop a vertical slice column on slow disc reads. | Add a visual indicator (brief static bar) if a PIO fallback triggers, so it's diagnosable. |
| **SE-4** | SPU Voices | 13 of 24 SPU voices are allocated to SFX, 3 to audio streams, 4 to the synth sequencer, leaving 4 free. If future features add environmental ambience or more SFX variety, voice contention could occur. | Document the voice allocation table in `audio.rs` header comments. Currently undocumented. |

### Stability

| # | Area | Issue | Recommendation |
|---|------|-------|----------------|
| **SE-5** | Controller Hot-Plug | Controller disconnect overlay freezes gameplay — excellent. But reconnection requires the game to re-init analog mode (`command 0x44`) and motor mapping (`command 0x4D`). | Verify that hot-plugging mid-game correctly re-enters analog mode on reconnection, and doesn't leave the controller in digital-only mode. |
| **SE-6** | Memory Card Edge Cases | `SaveStatus::SaveErrorFailed` is handled but no retry mechanism is offered — the message displays briefly and disappears. | Show "SAVE FAILED — RETRY? (CROSS/TRIANGLE)" prompt so the player can retry before the status clears. |
| **SE-7** | `i16` Overflow Risk | World coordinates use `i32` but are cast to `i16` for GTE projection (e.g., `wx as i16`). With `GRID_W=24 × TILE_SZ=64 = 1536`, maximum coordinate is 1536 which fits in `i16` (max 32767). Safe, but any future grid size increase would silently wrap. | Add a compile-time assertion: `const _: () = assert!((GRID_W as i32 * TILE_SZ) < i16::MAX as i32);` |

---

## 🎨 Product Designer — UX, Interface & Player Experience

### Onboarding & Tutorials

| # | Priority | Issue | Recommendation |
|---|----------|-------|----------------|
| **UX-1** | 🔴 **High** | **No in-game controls tutorial.** The game has 8+ distinct abilities (Crawl, Box, Sonar, Spur Strike, Dive, Jump) mapped to specific buttons, but no tutorial overlay or button prompt is shown outside of the CODEC conversations. New players will miss abilities entirely. | Add a brief HUD prompt on first use: e.g., "Press ○ to CRAWL" when approaching an AirDuct for the first time, "Press △ to charge SONAR" on Act 2-2 start, etc. |
| **UX-2** | 🔴 **High** | **No pause menu.** Pressing START during gameplay does nothing — there's no way to pause, view controls, adjust options, or quit to title mid-stage. SELECT opens the CODEC, but that's an in-game mechanic, not a pause screen. | Add a pause overlay on START with options: Resume, Controls Reference, Options, Quit to Title. |
| **UX-3** | 🟡 **Medium** | **Stage Clear screen lacks summary.** Completing a non-boss stage shows "STAGE CLEAR" then immediately loads the next CODEC briefing. No stats (time, score, collectibles found, alerts) are shown until the final debriefing after Act 4-3. | Show a brief stage summary card: Time, Score, Yabbies Found, Alert Status, and an optional "CODEC: Save?" prompt. |
| **UX-4** | 🟡 **Medium** | **Oxygen meter visibility.** The O2 bar only appears when in water. Players entering water for the first time (Act 1-1 canal) won't know they have limited air until they're already drowning. | Flash the O2 bar briefly with a "HOLD × TO DIVE — WATCH YOUR O₂!" text prompt when first entering water. |
| **UX-5** | 🟡 **Medium** | **No map or objective marker.** In larger stages (Act 1-2, 3-2, 4-1) the exit burrow location is unknown to the player. The Soliton Radar shows enemies but not the objective. | Add a subtle pulsing chevron or blip on the radar pointing toward the exit burrow. |
| **UX-6** | 🟢 **Low** | **Game Over retry is instant.** After dying, Cross/Start immediately reloads the stage with no loading screen or brief delay. While fast, it can feel disorienting. | Add a 0.5-second fade-to-black transition before the stage reload. |

### Interface Polish

| # | Priority | Issue | Recommendation |
|---|----------|-------|----------------|
| **UX-7** | 🟡 **Medium** | **Options are not accessible mid-game.** Costume, wireframe, language, and display settings can only be changed from the title screen Options menu. | Add an Options sub-menu inside the proposed pause menu, or at minimum allow costume swapping from the CODEC menu. |
| **UX-8** | 🟡 **Medium** | **Attract Demo plays the actual game state.** The attract demo simulates button inputs on real stages, which means the player entity, enemies, and scoring are all running live. If attract demo runs after a completed campaign (with unlocks), it loads VR stages due to PD-1. | Use a dedicated demo recording/playback system, or at minimum snapshot and restore the full game state before/after attract mode. |
| **UX-9** | 🟢 **Low** | **No volume control.** There is no option to adjust SFX or BGM volume independently. The synth sequencer and CD-DA have fixed volumes. | Add volume sliders (0-100%) for BGM and SFX in Options, controlling SPU voice master volume. |
| **UX-10** | 🟢 **Low** | **Debriefing stats only shown once.** The final mission debriefing (codename, time, alerts, damage) displays after Act 4-3 but cannot be reviewed from the title screen. | Add a "MISSION LOG" option on the title screen (when save exists) showing best codename, time, yabbies, alerts. |

---

## 📣 Marketing Team — Presentation, Polish & Sellability

### First Impressions & Visual Polish

| # | Priority | Issue | Recommendation |
|---|----------|-------|----------------|
| **MK-1** | 🔴 **High** | **No chapter title cards.** Transitioning from Chapter 1 to Chapter 2 (or any chapter) goes straight from CODEC briefing to gameplay. There's no dramatic chapter splash screen ("CHAPTER 2: THE YARRA RIVER WILDS") with artwork or a cinematic establishing shot. | Add a 3-second letterboxed chapter title card with the chapter name, location subtitle, and a brief environmental panorama before the first stage of each chapter. |
| **MK-2** | 🔴 **High** | **Boss introductions lack drama.** The `BossIntroCutscene` does a 4-second camera orbit, but there's no boss name title card ("PERIMETER WALKER MK-I"), no dramatic freeze-frame, and no unique boss music intro sting. | Display a cinematic boss name banner with a subtitle (e.g., "SEARCHLIGHT MECH — PERIMETER WALKER MK-I") during the orbit, with a dramatic percussion sting or rising synth. |
| **MK-3** | 🟡 **Medium** | **Ending scene may feel anticlimactic.** After the final boss, the outro video plays, then a debriefing stats card, then a static sunset ending. There's no credits sequence listing the developer. | Add a credits scroll over the sunset ending scene, or at minimum display "DEVELOPED BY [NAME]" and "THANK YOU FOR PLAYING" with the codename badge. |
| **MK-4** | 🟡 **Medium** | **VR Training completion has no fanfare.** Clearing all 4 VR stages unlocks Tuxedo and Wireframe, but there's no celebration screen — it just plays a quick fanfare and returns to the VR menu. | Show a dedicated "VR TRAINING COMPLETE — TUXEDO UNLOCKED! WIREFRAME UNLOCKED!" splash with the Platty model in Tuxedo doing a pose. |
| **MK-5** | 🟡 **Medium** | **Stealth Camo (Rank S reward) has no visual feedback.** The camo is unlocked but the player may not know what it does or how dramatic it looks until they equip it and replay. | Show a brief "STEALTH CAMOUFLAGE ACTIVATED" preview during the debriefing when Big Platypus rank is achieved, with Platty flickering semi-transparent. |

### Replay Value & Content

| # | Priority | Issue | Recommendation |
|---|----------|-------|----------------|
| **MK-6** | 🟡 **Medium** | **No stage select after completion.** Once the campaign is beaten, the only way to replay a specific stage is to start a new campaign and play through sequentially. | Unlock a "STAGE SELECT" menu option on the title screen after completing the campaign, allowing replay of any individual act. |
| **MK-7** | 🟡 **Medium** | **Collectible tracking is aggregate only.** `total_yabbies` accumulates across all playthroughs but there's no per-stage breakdown showing which stages have uncollected items. | Add per-stage yabby/letter counts to the stage select screen (if added) or mission log. |
| **MK-8** | 🟢 **Low** | **Only 5 codename ranks.** Games like MGS have 12+ codenames based on diverse playstyle combinations. Adding more ranks (e.g., "Pacifist Wombat" for 0 takedowns, "Ghost Platypus" for 0 alerts + 0 takedowns + 0 damage + no box used) would encourage varied replays. | Add 2-3 additional codename tiers for niche playstyles. |

---

## 🧪 Testing Developer — QA, Edge Cases & Regression

### Critical Test Gaps

| # | Priority | Area | Test Needed |
|---|----------|------|-------------|
| **QA-1** | 🔴 **Critical** | Continue After Completion | Test: Complete Act 4-3, return to title, select "Continue". Expected: Should NOT load VR-01 as a campaign stage. Actual: Loads `Act::from_u8(12)` = `VrSneaking`. |
| **QA-2** | 🔴 **Critical** | Boss Defeat + Retry Loop | Test: Die on a boss stage, retry, defeat the boss. Verify that boss HP, conduit states, and phase timers are fully reset. Specifically test Act 1-3 mech where 3 conduits + 4 core HP must all reset cleanly. |
| **QA-3** | 🟡 **Medium** | Oxygen Edge Cases | Test: Enter water with 1 HP, drain oxygen to 0. Verify damage is applied correctly (1/frame) and Game Over triggers. Test: Surface at air=1, verify recovery rate. Test: Submerge → surface → submerge rapidly (air flickering). |
| **QA-4** | 🟡 **Medium** | Cardboard Box + Boss | Test: Enter a boss fight while in the cardboard box. Verify: Can the box be toggled during boss encounters? Does the box provide any unintended protection from boss attacks? |
| **QA-5** | 🟡 **Medium** | Score Overflow | Test: Accumulate maximum score across a full playthrough with all collectibles and takedowns. Verify `u32` score doesn't overflow. Maximum theoretical: ~50,000 (collectibles) + ~25,000 (bosses) + ~10,000 (takedowns) = ~85,000. Safe for `u32`, but verify display with 6-digit formatting. |
| **QA-6** | 🟡 **Medium** | Memory Card Full | Test: Fill Memory Card Slot 1 with 14 other save blocks (leaving 1 block free). Save and verify. Then fill completely (15 blocks used). Attempt save and verify `SaveErrorFailed` displays. |
| **QA-7** | 🟡 **Medium** | River Rapids Boundary | Test: In Act 2-1, move to the extreme left and right edges of the river. Verify player cannot clip outside the 5-lane boundaries. Test: Jump at the exact moment an obstacle reaches the player Z. |
| **QA-8** | 🟡 **Medium** | Electro-Sonar + Bosses | Test: Fire electro-sonar pulse during each boss fight. Verify it doesn't stun bosses (it shouldn't — only stuns sentries and drones). Verify the visual overlay doesn't interfere with boss HUD elements. |
| **QA-9** | 🟢 **Low** | Attract Demo State Leak | Test: Let attract demo run, press a button mid-demo, return to title. Verify that score, health, entity states, and BGM are fully reset. |
| **QA-10** | 🟢 **Low** | PAL Timing | Test: Set video mode to PAL 50Hz. Verify that all gameplay timers (invulnerability, stun durations, oxygen depletion, boss attack cooldowns) are adjusted for 50fps, or document that the game runs 16.7% slower in PAL mode. |

### Existing Test Suite Gaps

| # | Missing Coverage | What to Add |
|---|------------------|-------------|
| **QA-11** | Boss state machine transitions | Verify `SearchlightMech` transitions through `Patrolling → Targeting → Stomping → Venting → Patrolling` correctly, and that destroying all 3 conduits triggers shield collapse. |
| **QA-12** | Collectible pickup effects | Verify `YabbyRation` heals +1 (capped at 3), `BuriedYabby` heals to full, `ChaffBattery` resets sonar cooldown, `CardboardBox` sets `has_box = true`. |
| **QA-13** | River obstacle wrapping | Verify obstacles correctly wrap from `z < 2*TILE_SZ` back to `z = 21*TILE_SZ` without gaps or overlap. |
| **QA-14** | `fixed.rs` dead code | `Fixed` struct is defined but never imported. Either add a unit test confirming its arithmetic or remove it. |

---

## Summary Priority Matrix

```
                        IMPACT
                 Low         Medium        High
           ┌──────────┬──────────────┬──────────────┐
    High   │          │  UX-1, UX-2  │    PD-1      │
           │          │  MK-1, MK-2  │              │
EFFORT     ├──────────┼──────────────┼──────────────┤
    Medium │  MK-8    │  PD-2, PD-3  │  MK-6, UX-5 │
           │  QA-14   │  UX-3, UX-7  │              │
           ├──────────┼──────────────┼──────────────┤
    Low    │  UX-6    │  PD-5, SE-7  │  QA-1, QA-2  │
           │  UX-9    │  MK-3, MK-5  │              │
           └──────────┴──────────────┴──────────────┘
```

> [!IMPORTANT]
> **Top 5 items to address first:**
> 1. **PD-1** — Continue after completion loads VR stage (bug)
> 2. **QA-1** — Write test to verify continue flow
> 3. **UX-2** — Add a pause menu (START button does nothing)
> 4. **UX-1** — Add contextual button prompts for abilities
> 5. **MK-1** — Add chapter title cards for dramatic pacing
