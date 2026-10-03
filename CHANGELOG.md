# Changelog

All notable changes to Plattypus are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.3] - 2026-10-03

### Fixed

- **FMV Playback & MDEC DMA Initialization:** Resolved a race condition where MDEC DMA0/DMA1 enables were swallowed by an ongoing MDEC reset on PS1 hardware and PSoXide emulator, causing quantization table uploads to fail and video playback to abort. Enforced proper bus wait cycles using volatile MMIO reads.
- **Web Drive Range Prefetching:** Synchronized sector prefetch readiness in PSoXide emulator when a drive seek/read target (`setloc_pending`) is scheduled.
- **Web Player Full Audio & Video Integration:** Master CD-ROM image with CD-DA audio and interleaved XA FMV streams now plays smoothly in the browser.

### Added

- **On-Screen Touch Controls:** Added authentic PlayStation 1 virtual touch controls (D-pad and action buttons) for mobile portrait/landscape gameplay with tactile haptics.
- **Stage Start Camera Rotation:** 3D camera pan around Platty at the start of each stage to showcase perspective 3D depth, quickly turning back upon input.

- **Intro video no longer stutters.** Presentation is now FIFO-buffered and
  continuous, CD-XA streaming never stops the drive, and every wait pumps the
  stream. A frame can no longer be presented twice or skipped.
- **Depth sorting, quad winding and internal wall culling** in the renderer.
- **Line of sight is correct and symmetric.** The corner-crossing test
  re-tested the tile the walk had already come from instead of the two tiles the
  line actually straddles, and the overshoot guard fired on the first step of any
  backwards walk, reporting those sight lines as clear. Both meant a guard could
  see through a diagonal squeeze between two walls, and the result depended on
  which endpoint was passed first.
- **The final boss can now hurt you.** Act 4-3 had no damage source at all: no
  sentries, no tripwires, and no hit test on either boss attack.
- **Ten of the twelve codenames are reachable again.** The rank was graded once,
  on the final mission, which is evaluated with no alerts, no damage and no
  takedowns -- so every run scored Rank S and the other ten ranks could never be
  earned. Ranks are now graded per stage and the best is carried as the campaign
  record.
- **The stage-select service records are visible.** They were drawn and then
  painted over by the footer's opaque background on the same screen; the columns
  also overlapped each other and ran off the right edge.
- **The title screen no longer prints two overlapping strings.** The footer and
  the save-status OSD shared an 18 px band, and starting a new campaign triggers
  a save, so the first button press produced unreadable overprinted text.
- **Music is audible as notes.** The synthesiser voices held sustain level `0xF`
  and the SPU has no retrigger, so each "note" kept looping while the pitch
  stepped between values -- one continuous oscillator rather than a melody.
- **The music no longer clips.** Three voices at unity main volume summed to
  2.13x of full scale, sustained, in every level.
- **The ground under the player no longer disappears on heavy stages.** The quad
  budget is spent far-to-near and the tile loop used to stop dead when it ran
  out, so a stage needing more quads than the budget allowed lost its nearest
  rows entirely.
- **The electro-sonar stun can no longer disable a stage permanently.** It
  ignored walls, reached five tiles, and stunned for longer than its own
  recharge, so one held button removed the entire detection layer.
- **Analog speed control works as documented.** A committed stick push travelled
  at the sneak speed while the player was classified as running, emitting full
  footstep noise at half speed; top speed needed the stick at absolute maximum.
- **The game now states its objective.** `Act::subtitle()` held sixteen mission
  briefings and was never called from anywhere.
- **The player model no longer blinks out of existence** for the first 1.5
  seconds of every stage.
- **The tutorial no longer teaches a dead button.** It told a new player to
  press CIRCLE *or DOWN* to crawl; only CIRCLE is bound.
- **The frame counter no longer wraps every 256 frames**, which made the title
  cursor pulse stutter and telemetry drift.
- **Debriefing counters saturate instead of wrapping**, so 142 alert phases
  displayed as `42`.

### Added

- `Plattypus Web Arcade`: a browser build of the disc, served by
  `make web` and published by GitHub Pages.
- `make iso` for a cooked ISO master alongside the raw `.bin`/`.cue` disc.
- Release and GitHub Pages deployment workflows.

### Changed

- **Asset provenance recorded.** All music and cinematics are Gemini-generated;
  `ASSETS.md` now documents the full set, including which generated visualisers
  are unused.
- **Licensing position made explicit.** This project is unlicensed non-commercial
  homebrew with copyright retained, and is not for sale. `LICENSE` now states
  that plainly, and no document describes a retail programme any more. No
  commercial use, resale or retail distribution is authorised or endorsed.
- The PSoXide SDK is vendored in-tree (`psoxide/`) instead of being a git
  submodule, so a plain `git clone` is enough to build.
- Save integrity, campaign progression and the score display were reworked.
- `plattypus-core` now owns the level generators, so the host test suite
  exercises the real code instead of a mirror of it.

## [0.1.0] — Unreleased

First playable build.