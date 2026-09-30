# Why intro-video playback is stuck at 6 fps, and what would actually fix it

Investigation note. The measurement harness in `tools/video_bench/` is the
evidence; this file is the argument. Nothing here is implemented: the tree
is at the measurement commit, and the optimisation attempt that produced
these numbers was reverted because it did not improve the measured result.

## The budget, stated once

The video is 150 frames at 15 fps, so each frame has exactly **4 display
periods, 66.67 ms**, to be read, decoded, uploaded and presented.

Measured per frame (`docs/perf/baseline-intro-video.json`):

| | ms | note |
| --- | --- | --- |
| CD read | 71.22 | 16,384 bytes; amortised over a 4-frame batch |
| MDEC decode | 25.89 | 20 `drain_slice_dma` |
| VRAM upload | 12.11 | 20 GP0 transfers, 153,600 bytes |
| **total** | **110.22** | **661% of the 66.67 ms budget** |

The drive itself is not the problem. Read contiguously it sustains
**130 sectors/s**; the video needs 120. There is headroom.

## Three fixes that were built, measured, and do not reach 15 fps

Each was implemented on a branch, measured with the bench, and reverted.

**1. Kill the per-batch seek (measured: read 71.22 -> 58.55 ms).**
The `cdrate` probe shows the drive delivers 130 sectors/s contiguously but
only 62 in the shipped per-batch pattern: a `Setloc` costs about as much as
the eight sectors it then fetches. Raising the read-ahead from 4 to 16
frames (one 128-sector burst) recovered that, reaching ~137 sectors/s --
at or above the drive's own contiguous rate. This is a real, kept-worthy
win on read bandwidth, and it did not move the frame rate.

**2. Decouple the present from the decode.** `begin_frame()` swaps
*before* `draw()`, so the decode gets one display period to do 110 ms of
work while the next present is four periods away. Presenting only when a
completed frame is waiting gives the decode the full four-period window.
Structurally correct, and the right architecture to keep. Measured: no
change (6.00 fps).

**3. Continuous streaming with a ring buffer, read moved off the draw
path.** Intended to let the drive stream while the MDEC works. **This is
where it stops, and the reason is not a performance problem.**

## The MDEC failure: corrected mechanism

The earlier version of this section was wrong, and the correction is the
useful part. Two things were misread:

- **`0x9604FFFF` is not a failure.** Decoded against `psx_hw::mdec::status`
  it is `DATA_OUT_EMPTY`(31) set, `DATA_IN_FULL`(30) clear, `BUSY`(29)
  clear, `DATA_IN_REQ`(28) clear. It is the normal idle state after a
  frame has been fully drained. The whole "MDEC is backed up" reading was
  built on that.
- **"The MDEC cannot decode while a ReadN is live" is not true.** A probe
  (`stages::probe_mdec_contention`) decodes one real frame under six
  different ways of parking the drive -- live, `pause_read`,
  `pause_read`+`ack_all`, full `stop()`, ack+quiesce, and quiesce -- with a
  genuine `ReadN` running and a sector already in the FIFO:

  ```
  case=live         slices=20/20
  case=pause        slices=20/20
  case=pause+ack    slices=20/20
  case=stop         slices=20/20
  case=ack+quiesce  slices=20/20
  case=quiesce      slices=20/20
  ```

  All six succeed. The drive's state during a decode is irrelevant.

### What actually happens

The failure is cumulative, and it needs the CD to be *active across*
frames rather than merely live. `stages::probe_mdec_repeat` reads a frame
from a running stream and decodes it, over and over:

```
iter 0  slices=20/20  settle=0      busyafter=0
iter 1  slices=20/20  settle=2000000 busyafter=1   <- MDEC wedged BUSY
iter 2  slices=18/20                             <- and now it fails
```

The settle counter is a spin budget spent waiting for the MDEC to go idle
after the 20th slice. On iteration 1 it exhausts 2,000,000 spins with the
MDEC still BUSY. The end-of-frame status there is `0x3E040082`:
`BUSY` + `DATA_IN_REQ` + `DATA_OUT_REQ` -- the MDEC is **waiting for input
that never arrives**, with its input channel (ch0) already idle. Draining
more does not help (`extra=0`: no output is pending, 20 slices really is
the whole frame), and `mdec::init()` before every frame does not prevent
it.

And the control: `stages::probe_mdec_nocd` decodes the same frame 40 times
with the drive stopped throughout:

```
iter 0..39  slices=20/20  settle=0  busyafter=0   stat=9604FFFF
```

Perfect. The MDEC is entirely happy to decode back-to-back frames.

### The mechanism, and why the old code survived

So the wedge requires **sustained CD activity overlapping the decode**, and
it presents as the MDEC's input DMA failing to deliver the frame. A single
overlapped frame has enough slack for ch0 to finish; sustained overlap
starves it.

The old batched reader never hit this because it **stopped the drive around
every decode**, and the ~200 ms read burst between decodes gave the MDEC
time to go idle on its own. The wedge was masked by timing, not prevented.
The new staggered scheduler reads during the gaps, so the decode no longer
has that quiet window, and the latent problem surfaces.

This is a better-defined problem than "the MDEC and the CD conflict": it is
the MDEC input DMA not completing under sustained bus activity, and it is
reproducible in a 3-iteration loop with no game code involved.

What would fix it, in the order I would try:

1. **Give the decode a quiet bus again, but keep the stagger.** Present the
   frame that is already decoded, then read, and only decode once the read
   for the *next* frame is finished. That serialises read-then-decode per
   frame but no longer blocks a ready present, which is the part that
   actually costs frames. It should recover most of the 37 late presents
   without needing the MDEC to tolerate the drive.
2. **Pace the input explicitly**: after `start_decode_frame`, wait for ch0
   to go idle before draining, so the MDEC is never asked to output while
   its input is still arriving. Cheap to try, and it is the direct
   counterpart of the observed `DATA_IN_REQ`.
3. Only then, the bitrate and the channel-2 upload already in place.

## What the arithmetic says is actually required

The drive needs **58.55 ms of CD-live time per frame** (at the
16-frame read-ahead's 137 sectors/s). The MDEC needs the CD **off** for
**25.66 ms** of that. So:

```
CD-live time available per frame = 66.67 - 25.66 = 41.01 ms
CD-live time required            = 58.55 ms
```

Two ways out, and only two:

**A. Solve the contention.** Then read and decode overlap, and the frame
cost becomes `max(58.55, 38.33) = 58.55 ms <= 66.67 ms`. **No bitrate
change needed, 15.00 fps reachable.** Worth trying first: the decode
itself is suspect as a measurement, since 25.66 ms for 320x240 is
3.0 Mpixel/s where silicon does roughly 12-15. DuckStation's MDEC timing
looks 3-5x slow, which is a reason to believe the contention is an
artefact of its model rather than of the hardware -- but that cannot be
settled without hardware, and it cannot be shipped on the strength of a
guess.

**B. Cut the bitrate to ~70% of current** (16,384 -> ~11,500 bytes/frame,
8 -> 5.6 sectors), which fits the read into the 41 ms that is available
even with the contention in place. This is a content change: re-encode
`Videos/INTRO.VID` with a coarser quantiser via
`tools/encode_mdec_video.py`, accepting a visible quality loss on a
90-second opening cinematic.

Note that option B is only 70%, not 48%, *because* the decoupled present
already bought back the four-period window. Without fix 2 the number is
48% and the quality cost is far worse -- so fix 2 is a prerequisite for
the cheap version of fix B, even though it is worth nothing on its own.

## What is not available

- **DMA VRAM upload.** `dma_copy_to_vram` exists and would remove the
  12.11 ms upload outright, but `psx-vram` documents that channel 2 can
  wedge busy-forever on real silicon (CL2 probe, 2026-07-31) and that its
  completion wait then spins unboundedly. Not a safe default for a boot
  path.
- **Overlapping the decode with the upload.** `drain_slice_dma` blocks,
  so this needs a non-blocking arm in `psx-io` -- an SDK change. Even
  done, it only takes compute from 38.33 to ~25.66 ms, which does not
  change the conclusion above, because the read is the binding term.

## Recommendation

Try A first, on hardware or on an emulator whose MDEC timing is trusted,
before touching the video assets. If the contention turns out to be real
silicon behaviour rather than an emulator artefact, B is the remaining
route and it is a content decision, not an engineering one -- so it
should be made deliberately, with the frames eyeballed.


## Appendix: the bitrate cut was tried, and it did not work

Option B was implemented and measured rather than assumed.

**A variable-length container was built first**, because a fixed frame
slot caps the cut far lower than 30%. Measuring the floor payload of every
frame (coarsest legal quantisation) gave mean 8,731 B but max 12,548 B, so:

- fixed 7-sector slot: 87.5% of the original (a 12.5% cut) -- works,
- fixed 6-sector slot: frame 77 does not fit at all,
- variable container: 62.7% of the original (a 37% cut).

The encoder gained `--variable-sectors` (a 1-sector header of per-frame
counts) and `--target-words` (a typical-frame budget with overflow allowed),
and its adaptive quantiser search became a bisection, since payload is
monotonic in scale -- the linear scan re-encoded each frame up to 25 times
and made a 150-frame video take 15+ minutes; the bisection does it in about
a minute. The bisection picks the same scale as the linear scan except
where it can find a *finer* scale that still fits, which is an improvement.

**The variable container works and decodes all 150 frames** (756 data
sectors, mean 5.04/frame, max 7), but its per-frame read count made the
paced run wedge intermittently in the CD reader: a batch read of a
variable number of sectors, with the drive paused at the end, does not
always drain before the next command, and DuckStation then reports
"Interrupt not processed in time, missed sectors" and the reader hangs.
The fixed-stride reader has no such failure mode, so the measurement below
uses the fixed 7-sector slot.

**Result, fixed 7 sectors/frame, 1050 sectors vs 1200 (12.5% cut):**

Measured with the shipped `PREFETCH_FRAMES=4`, the only change being the
bitrate:

| | baseline (8 sectors) | after (7 sectors) |
| --- | --- | --- |
| presented fps | 6.00 | **6.67** |
| interval mean | 10 vb | **9 vb** |
| stutters | 149/150 | 149/150 |
| avg frame time | 166.7 ms | **150.0 ms** |

A real but small improvement: 6.00 -> 6.67 fps, the mean interval drops from
10 to 9 display periods. Far short of the 15 fps target, and the arithmetic
says why it could not have been otherwise. With read and decode *serialised*
-- which is the shipped architecture, since the MDEC contention that would
let them overlap is unresolved -- the frame cost is `read + compute`. Twelve
percent off the read is ~7 ms off a ~97 ms total, and the drive must still
deliver its sectors inside the one display period the decode has. Cutting
data does not touch the term that is actually binding, which is the
serialisation itself.

(A `PREFETCH_FRAMES=1` variant measured 5.00 fps -- worse. That is the
per-batch seek and burst overhead growing relative to the smaller frame, not
the bitrate, and is why the committed value is back at the shipped 4.)

So option B on its own is worth about 0.7 fps. It is not the fix; the fix is
still to let the read and the decode overlap, which needs the MDEC/CD
contention resolved. This commit keeps the 7-sector encoding only because it
is a strict improvement over the 8-sector baseline at no code risk -- not
because it changes the picture.

The bench now emits the paced metrics immediately after the paced phase
(`@@VB1 paced_only ...`), so a wedge in a later diagnostic phase cannot cost
us the headline numbers.


## Where DMA is available and unused

Inventory of the six root DMA channels against what the project actually
uses, and what each unused one is worth.

| Channel | Used for | Status |
| --- | --- | --- |
| 0 | MDEC in (RAM to MDEC) | used, `start_decode_frame` |
| 1 | MDEC out (MDEC to RAM) | used, `drain_slice_dma` |
| **2** | **GPU (RAM to VRAM)** | **`dma_copy_to_vram` exists, has zero call sites** |
| 3 | CD-ROM (drive to RAM) | channel is *enabled* in `SectorReader::prepare` but never started; the reader pops 2048 bytes per sector over PIO |
| 4 | SPU | used |
| 5 | PIO | n/a |
| 6 | OTC (ordering-table clear) | used |

### Channel 2 is the real one: 11.11 ms/frame, 88% faster

Measured directly, same payload and same destination geometry, 150 frames'
worth, with no CD read or decode in the way (`stages::measure_upload_paths`,
emitted as `@@VB1 upload fifo_us=... dma_us=...`):

```
GP0 FIFO (shipped) :  12.55 ms/frame   = 753% of one display period
DMA ch2 (unused)   :   1.44 ms/frame   =  87%
saving             :  11.11 ms/frame  (88% faster)
```

The shipped path pushes 38,400 words per frame through the GP0 command
port one word at a time. The DMA path hands the same bytes to channel 2 in
block mode. It is fully implemented and `pub`; nothing calls it, which is
why it has sat there.

It passes the function's own safety guard for this geometry: a 16x240
slice is 8 words per row, inside the 16-word GPU FIFO limit the guard
enforces. The reason nothing calls it is documented in `psx-vram` and is
worth repeating here: on real silicon a probe on 2026-07-31 found channel 2
can latch its start bit and stay busy forever, and the completion wait would
then spin unboundedly. That is a hardware-safety finding, not a performance
one, and this measurement is from DuckStation, so it cannot speak to it.
**Enabling it is a real-hardware risk decision, not a code decision.**

What it buys: compute drops from 38.0 ms to 27.3 ms per frame. On its own,
against a serialised read of ~51 ms at seven sectors, that moves 6.67 fps
to roughly 7.5 -- worth having, still not 15. Combined with the overlap that
the MDEC contention fix would unlock, it is *necessary*: with the read and
decode overlapping, the frame cost becomes `max(read, decode + upload)`, and
27.3 ms against a 66.67 ms budget clears comfortably where 38.0 does not.

### Channel 3 is not worth pursuing

The obvious-looking one, and a trap. The reader does 2048 byte-at-a-time
MMIO pops per sector where a channel 3 transfer would move the whole sector
at once, and `dma_read_sector` is still *named* for the DMA recipe it no
longer uses. But the measurement says the read is already at the drive's
limit: 137 sectors/s sustained against 130 measured for a pure contiguous
stream. The pops are already hidden behind the drive wait, so channel 3
would save approximately nothing -- which is exactly what the SDK's own
note predicted ("~1.3 ms slower per sector than a working DMA, which no
SectorReader user notices").

And channel 3 is the one path the same real-silicon probes convicted: it
could read as all-zero sectors everywhere. Trading a measured zero saving
for a known data-corruption failure mode is a clear no.

So of the two unused channels, one is worth 11 ms/frame and is blocked only
by a hardware risk the project already knows about, and the other is worth
nothing and carries a known correctness risk.


## 10 fps attempt: reached the rate, not the smoothness

Per the follow-up plan: drop the target to 10 fps, then try a RAM cache.

**10.00 fps is reached.** Three changes, all measured:

1. **Decoupled present.** `begin_frame()` swapped before `draw()`, so the
   decode got only the window between one swap and the next and any
   overrun pushed the following swap out by a whole display period. Now a
   frame is presented only once it is complete, so the decode gets the
   full window. `present()` also starts the decode as soon as the back
   buffer is free, immediately after the previous present, rather than
   when the next frame falls due -- waiting for the due time made each
   period `target + decode_time` instead of `max(target, decode_time)`.
2. **A floor on the dwell time.** Without it the cadence followed the
   decode rather than the target: a 1.7-period decode was being presented
   every 2 display periods (~30 fps) and then the read frames lurched.
   With the floor, 112 of 150 presents land on exactly 6 display periods.
3. **DMA channel 2 for the VRAM upload** (previous commit), which took the
   upload from 12.55 ms to 1.55 ms.

Result: **10.00 fps**, interval 6 vb on 112 of 150 presents, pipeline
82.89 ms/frame. Against the 6.00 fps baseline that is a 1.67x improvement
and 23 fewer milliseconds of CPU per frame.

**But it is not smooth, and so is not being shipped as smooth.** 37 of
150 presents still land at 17-19 display periods. The histogram is bimodal:
112 at 6 vb and 37 at 17-19, and the 37 are exactly the frames served
from a CD read-ahead burst (every fourth frame, at `PREFETCH_FRAMES=4`).

### Why no cache size fixes it

The arithmetic, with the numbers above:

```
video duration            150 frames / 10 fps     = 10.0 s
drive time   1050 sectors @ 130 sect/s contiguous =  8.1 s
CPU  time     (25.66 decode + 1.55 upload) x 10   =  0.3 s
serialised                                        =  8.3 s   -> fits, 1.7 s spare
```

It fits on *totals*. It does not fit on *shape*: the read is 54.89 ms per
frame of drive time against a 27.2 ms compute, and single-threaded that
can only be hidden by running the drive during the decode -- which is the
MDEC contention, still unresolved. So the read lands as blocking bursts and
every burst is visible as late presents.

A RAM cache removes the read from the frame path, but only for the frames
it holds. Measured RAM headroom is 844 KiB and a 7-sector frame is 14 KiB,
so at most 56 of 150 frames (37%) can be cached; the whole video is
2,100 KiB and neither it nor half of it fits. So the best a full-memory
cache can do is make 37% of the video perfectly smooth and leave the rest
stalling. An attempt to build exactly that (a 56-frame ring, drive stopped
around each decode, refilled in the gaps) primed all 56 frames and then
reproduced the contention as soon as the stream resumed mid-playback, which
is the same wall from a different direction: only `SectorReader::stop`
satisfies the MDEC, not `pause_read`, and resuming a stopped reader means
a seek.

### What would actually make it smooth

In order of leverage:

1. **The MDEC/CD contention.** Worth 54.89 ms/frame of overlap, and it is
   the difference between 8.3 s serialised and ~5.5 s overlapped. This is
   the only change that makes the *shape* right rather than just the
   total.
2. **A variable-length container** (already built and working, 37% cut,
   mean 5.04 sectors/frame) would cut the drive time to 5.1 s, which
   combined with a partial cache could make 10 fps genuinely smooth without
   touching the contention. It needs its CD-read wedge fixed first, though.
3. **A lower frame rate still** (8 fps = 7.5 periods) would absorb the
   bursts, at the cost of the intended 15 fps look.

Not shipped: 10.00 fps with 37 late presents is a real improvement over
the 6.00 fps baseline and is committed, but it does not meet the "smooth
playback" bar, and presenting it as shipped would be overclaiming.


## Staggering the load against the decode: measured

The proposal was to interleave the stages -- load x frames from the disc
into an in-memory cache, present a frame that is already ready, decode the
next, present again -- rather than serialising them.

**That structure is now in place, and it is not enough on its own.** The
scheduler in `present()` now runs in three phases per display period:

1. present a decoded frame that has served its dwell time (one register
   write, never delayed);
2. if the back buffer is free, decode into it -- reading first only if the
   cache cannot supply the frame;
3. if a decoded frame is waiting out its dwell, spend that idle time
   reading the frames after it.

Phase 3 is the stagger proper: the CD is touched when nothing needs
presenting, so a ready frame is never held up by a read.

### The finding: the totals fit, the burst shape does not

```
per-frame read cost        54.89 ms = 3.29 vb
decode + upload + copy     27.99 ms = 1.68 vb
                           -----------
amortised per frame        82.88 ms = 4.97 vb
dwell window at 10 fps                 6.00 vb   -> 1.03 vb spare
```

So on average a frame needs 4.97 of its 6 display periods. But the read is
not spread: it arrives as one synchronous burst of `PREFETCH_FRAMES`
frames, and during a burst the CPU cannot present at all. So the *burst*
has to fit the window, not the average:

| PREFETCH | burst | fps | mean interval | on-time |
| --- | --- | --- | --- | --- |
| 1 | 3.3 vb | 6.00 | 10 vb | 0/150 |
| 2 | 6.6 vb | 6.67 | 9 vb | 75/150 |
| 4 | 13.2 vb | **7.50** | **8 vb** | **112/150** |

The on-time fraction is exactly `(P-1)/P`: **one frame per burst is late,
whatever the batch size**, because the burst is atomic from the CPU's point
of view. Small batches fit the window but pay a seek every frame, which is
worse; large batches amortise the seek but overrun the window. Four is the
measured optimum, and 4 is what ships.

The 37 late presents at P=4 are those burst frames: 150/4 = 37.5 bursts,
37 late frames.

### What the stagger is actually waiting on

Phase 3 only works if the drive can stream *while a frame sits decoded*,
without a seek per frame -- which means keeping the `ReadN` live across
calls. That is the same live-ReadN condition the MDEC cannot decode
against, so phase 3 currently has to re-seek per batch, and that seek is
precisely what makes small batches expensive.

So the stagger is blocked by the same single thing as everything else:

- a live `ReadN` in phase 3 (no per-frame seek, burst spread over the
  dwell) needs the MDEC to tolerate a live drive, which it does not;
- without phase 3, the read must be a synchronous burst, and a burst
  longer than the dwell window costs one late frame per burst.

Removing the seek (phase 3 working) would take the read from 3.29 vb of
atomic burst to a trickle inside the dwell, and 150/150 on time at 10 fps
follows directly. That is the same MDEC/CD contention, now with the
scheduler drawn around it.

## Correction: the CD traffic threshold is not where `probe_mdec_repeat` implied

A sweep probe (`stages::probe_mdec_sweep`) was added to find how much CD
traffic can precede a decode. It steps the number of sectors read before each
decode -- 0, 1, 2, 4, 7, 14 -- and decodes a *different* frame every
iteration.

Result, six distinct frames per step, drive live throughout:

```
k=0  ok=6/6  settle=0  stat=9604FFFF
k=1  ok=6/6  settle=0  stat=9604FFFF
k=2  ok=6/6  settle=0  stat=9604FFFF
k=4  ok=6/6  settle=0  stat=9604FFFF
k=7  ok=6/6  settle=0  stat=9604FFFF
k=14 ok=6/6  settle=0  stat=9604FFFF
```

Fourteen sectors of live CD traffic immediately before a decode does not wedge
the MDEC. Combined with `probe_mdec_frames_no_cd` (eight distinct frames,
drive stopped, 8/8 clean), neither varying frame data nor sustained CD
activity is sufficient on its own to produce the stall, which contradicts the
reading of `probe_mdec_repeat` recorded above.

The first version of the sweep was wrong in a way worth recording: it held
the decoded frame constant, and with the frame constant the decode duration
is constant. Every `k` passed, but for the uninteresting reason that the
failing condition was never generated. Varying the frame as well is what
makes the result mean something.

`probe_mdec_repeat` still wedges at iteration 1 in the same build and still
passes when moved last in the probe order, so it is not ordering. Its loop
differs from the passing sweep only in that it reads the seven sectors it is
about to decode rather than discarding them into scratch. That difference is
unexplained. It does not affect shipped playback, which decodes all 150
frames without wedging.

What this does change: chunked overlap of read and decode is not blocked by
the drive. Whether the shipped player can exploit it is now a question about
the scheduler, not about the MDEC.
