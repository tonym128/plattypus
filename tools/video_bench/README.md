# Intro-video performance measurement

`make video-bench` measures the performance of the Plattypus intro video
playback and prints a report. This document defines what each number
means, why it is measured the way it is, and what the baseline says.

## Running it

```sh
make video-bench                  # one run
make video-bench RUNS=3           # three runs, median reported
make video-bench SECONDS=240      # raise the per-run wall-clock budget
```

Requirements: a PS1 emulator that logs BIOS TTY output to stdout.
DuckStation is used headless (`-nogui`) and is found automatically. Pass
`--emulator PATH` to override. Output is written to
`dist/video_bench_report.json`.

A full run takes roughly 60 s of emulated time and needs a `--seconds`
budget above that (the default is 150).

## What is measured, and how

`video_bench/` is a bare-metal PSX-EXE that textually includes
`game/src/video.rs` and `game/src/audio.rs` and drives them with the same
tick sequence `game.rs` uses for `GameState::IntroVideo`. It is not a
reimplementation: the code under measurement is the shipped code. The
only stand-in is a `renderer::Renderer` shim holding a real `FrameBuffer`,
because `video.rs` reaches through `crate::renderer` for exactly two
things — the VRAM Y of the buffer being drawn into, and `fb.swap()`.

The bench runs five phases.

| Phase | What it does |
| --- | --- |
| `probe` | Works out which cycle counter the target actually has. |
| `paced` | Plays all 150 frames with the game's own pacing. **The headline numbers come from here.** |
| `burst` | Runs the pipeline with no display sync and a stopwatch per stage, for attribution. |
| `cdrate` | Measures what the CD path can deliver, to separate drive limits from seek costs. |
| `real` | Times the shipped `draw()` again under burst conditions, as a closure check on `burst`. |

### Clocks

Two clocks, chosen at run time.

**VBlank count** (`psx_rt::interrupts::vblank_count`) is driven by the
VBlank interrupt: one unit is one display period, 1/60 s. It cannot
wrap or alias, and every emulator and real silicon gets the display model
right. Every headline metric — frame rate, frame timing, stutter count,
average frame time — is derived from this counter and nothing else, so
those numbers are valid wherever the bench runs.

**A 33.8688 MHz cycle counter** built from root counter 0 plus its
overflow register, used for sub-display-period attribution. This one is
probed, not assumed. On the development target (DuckStation) RCNT at
`0xBF80_1002` is unimplemented and the timer overflow register does not
count, so the bench selects the VBlank clock and reports `clock=vblank`.
The report's `probe` lines show what was found, so a run can never
silently present 16.67 ms data as if it were finer. On hardware where the
overflow register works, the same code selects `clock=cycles` and the
stage figures become per-sample exact instead of averaged.

When only the VBlank clock is available, sub-period resolution comes from
averaging: a stage summed over 150 frames resolves to 1/150 of a display
period, about 111 µs. That is finer than any stage that matters here.

## Metric definitions

### Frame rate, frame timing, stutters

The video is 15 fps on a 60 Hz display, so a correct present interval is
exactly **4 display periods**.

`FrameBuffer::swap()` is called from `begin_frame()` *before* `draw()`
writes the next frame into the newly-current back buffer. A frame is
therefore on screen for exactly as long as the gap between two swaps, and
`draw()` must complete inside that same gap. `draw()` starts immediately
after a swap, so it has **one display period** of uninterrupted time
before the next swap's `wait_vblank()`. Overrun it by any amount and the
following tick's `wait_vblank()` has already missed its edge, pushing
that swap out by a whole display period.

So:

- `interval` — display periods between consecutive presents. Target 4.
- `stutters` — presents where `interval != 4`. This is what a viewer sees
  as a repeat or a hold.
- `work` — display periods `draw()` consumed. `work >= 1` means `draw()`
  overran, which is the *cause* of the stutter; a frame with `work == 0`
  and `interval == 5` was a scheduling hiccup elsewhere. Both are reported
  so a fix can be aimed at the right thing.
- `fps` — `60 / mean interval`, so 15.00 is a perfect run.
- `avg_frame_time_us` — `mean interval` × 16.67 ms.

The present-interval histogram is the most diagnostic output. With a
4-frame read-ahead it comes out strongly bimodal: frames served from the
cache are cheap, and the one frame in four that triggers a batch read
absorbs the whole read.

The first present has no predecessor, so it records no interval. It still
counts towards `presented`, but it is excluded from the distribution and
from the stutter count rather than being reported as a stutter.

### Read time and stage cost

`burst` runs the pipeline with no display sync and a stopwatch around each
stage, over all 150 frames:

| Stage | What it covers |
| --- | --- |
| `read` | `SectorReader` pulling a four-frame batch, seek included |
| `copy` | `frame_cache[slot] -> frame_words`, a 16 KiB copy |
| `decode` | `start_decode_frame` plus twenty `drain_slice_dma` |
| `upload` | twenty `upload_words` into VRAM |
| `pipeline` | the sum of the four |

`budget_pct` expresses a stage against the **one display period** that
`draw()` actually has, which is the 100% line that matters.

`burst` is a second copy of the pipeline, so it can drift from the game.
`real` times the shipped `VideoPlayer::draw()` under the same conditions
and the report prints `closure_gap_pct`, the difference between the two.
Treat the stage split as untrustworthy if that gap is large.

### CD throughput

`cdrate` separates the two reasons a read can be slow:

- **contiguous** — 128 sectors in one `Setloc`+`ReadN`, drive left running.
- **batched** — 32 sectors as four batches of eight, re-seeking and
  pausing the drive after each, exactly as the game does.

75 sectors/s is a 1x drive, 150 is 2x. `contiguous / batched` near 1.0
means the drive is the limit and the fix is to read less or overlap
reading with playback. A ratio above ~1.3 means the per-batch seek is
what costs, and the fix is to stop re-seeking and stopping.

### Decode integrity

`integ` decodes a streamed frame and reports a checksum and a count of
non-zero decoded words. Without it, a run that "finished fast" because it
silently fell back to the 16-frame embedded loop, or decoded nothing, would
look like an improvement.

## Baseline

Recorded in `docs/perf/baseline-intro-video.json`, DuckStation headless,
1 run. **This is the state before any optimisation work.**

```
frames presented      150
presented fps         6.00      (target 15.00)
frame interval        min 5, mean 10, p95 24, max 24 vb   (target 4)
avg frame time        166.66 ms (target 66.67 ms)
stuttering presents   149 / 150
draw() overruns       150, worst 20 vb

per frame, burst:
  CD read              71.22 ms   427% of budget
  cache->work copy      1.00 ms     5%
  MDEC decode          25.89 ms   155%
  VRAM upload          12.11 ms    72%
  pipeline total      110.22 ms   661%
  shipped draw()      108.00 ms   closure gap 2%

CD throughput:
  contiguous           130 sect/s   (128 sectors / 59 vb)
  shipped batched       62 sect/s   (32 sectors / 31 vb)
  ratio                 2.10x       seek dominates

present-interval histogram:
    6 vb  111 frames    <- served from the 4-frame cache
   22 vb   18 frames    <- the frame that triggers a batch read
   24 vb   18 frames
```

### What it says

**1. The 4-frame read-ahead is quantised into the frame rate.** The
histogram is bimodal at 6 and 22-24 display periods. 111 of 150 presents
land at 6 (cache hits, one period late) and 39 land at 22-24 (cache miss,
the read absorbed into the present). Mean interval 10 means the video runs
at 6 fps instead of 15 — 2.5x slow motion, and every frame is late.

**2. The per-batch seek costs half the read bandwidth.** The drive
delivers 130 sectors/s when read contiguously but only 62 in the shipped
per-batch pattern. Each batch issues a fresh `Setloc` and then pauses the
drive with `stop()`; that seek is roughly as expensive as the eight sectors
being fetched. This is the single largest identified cost and it is
addressable without touching the bitrate.

**3. VRAM upload alone is 72% of the budget, decode is 155%.** Even with
free CD reads the pipeline does not fit in one display period. `upload`
is 20 GP0 FIFO transfers of 1,920 words each — 153,600 bytes per frame
pushed through the command port, which is what limits it. `decode` waits
on twenty sequential `drain_slice_dma` calls, each of which must complete
before the next is requested.

**4. The one-display-period budget is itself the structural problem.**
`begin_frame()` swaps *before* `draw()`, so `draw()` gets one display
period to do about 110 ms of work. The next present is four display
periods away — there is room for the work if it is not forced to complete
before the swap. Reordering or decoupling the swap from the draw is a
larger change than tuning any single stage, and no amount of stage
optimisation makes 110 ms fit in 16.67 ms.

### Caveats

- The stage split and CD rates come from DuckStation's timing and CD
  models. The paced-phase numbers, being VBlank-derived, are what to trust;
  the stage figures indicate *where* time goes, not cycle-exact silicon
  costs. On hardware, the cycle clock is selected automatically and the
  stage numbers become exact.
- `read` is charged per frame, but a batch read happens once every four
  frames, so `read_us` is the amortised average. The `burst.read_pct` of
  427% is therefore a per-frame amortised figure, and the per-present
  `read_vb` column in the JSON attributes the whole batch to the frame that
  triggered it.
- One run is a sample, not a distribution. `RUNS=3` and the median in the
  report are there for when a difference is small enough to need them.
