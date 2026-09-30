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

## The blocker: the MDEC cannot decode while a ReadN is live

With a CD `ReadN` running, the MDEC's block-mode input DMA (channel 0)
stops partway through a frame and the decode dies at about **slice 12-14
of 20**. Register trace of the failing frame:

```
f=8 col=11 pre=2E0402D4 inb=1     <- input DMA still running at slice 11
f=8 col=13 post=9604FFFF          <- dead
"no more pixels f=8 col=11 stat=9604FFFF w0=38000DC0 avail=8"
```

`0x9604FFFF` decoded against `psx_hw::mdec::status` is **not** what it
first appears: `DATA_OUT_EMPTY`(31)=1, `DATA_IN_FULL`(30)=**0**,
`BUSY`(29)=**0**, `DATA_IN_REQ`(28)=**1**. The MDEC is not backed up, it
is *starving* -- actively requesting input while the input channel is
idle. The DMA under-delivered. An earlier reading of this word as
"DATA_IN_FULL set" sent the investigation down the wrong path; the
corrected decode is what redirected it.

The sharpest single piece of evidence: **frames 0-7 always decode, frame 8
always fails.** Frames 0-7 come from the blocking prime at startup, read
back to back with no decode in between. Frame 8 is the first frame read by
the non-blocking `service_cd` path, i.e. the first one that arrives after
the ring has filled and the drive's data FIFO has backed up. So the
trigger is *CD state during the decode*, not DMA arbitration.

### What was tried, and what each one ruled out

| Attempt | Result | Ruled out |
| --- | --- | --- |
| Feed the MDEC via its data port from the drain's service callback | 150/150 frames fail (callback fires every 1024 spins, cannot keep the input FIFO full) | Not a ch0-DMA problem |
| `CHCR` mode sweep against a zeroed buffer | "20 slices" was a false positive: ch1 completes trivially with no input | The shipped `0x01000201` SyncBlock mode is fine; a SyncRequest "fix" was a red herring |
| `mdec::init()` before every frame | No change on its own | Not MDEC latch state |
| Mask the idle CD DMA channel (DPCR bit 27) | No change | Not DMA channel arbitration |
| Clear the drive's interrupt-enable mask, keep streaming | 0 failures, but the run then **stalls** -- the ring-full path set `stream_closed` and never resumed, so no further reads happened and it fell back to the old behaviour | Not (only) drive interrupts |
| `pause_read` around each decode | Still fails at frame 8, with the drive confirmed paused (`pause ok=1`) | **`pause` is not enough; `stop` is** |
| `psx_io::irq::ack(CDROM)` before the decode | Still frame 8 | Not the outstanding CPU `I_STAT` bit alone |

`SectorReader::stop` is `pause_read` **plus** `ack_all()` plus clearing the
deferred-sector state, and only `stop` is known to make the decode work.
So the requirement is narrow and precise: the decode needs the reader in
its fully-stopped state, and `pause_read` does not reach it. Finding what
in that delta the MDEC is actually sensitive to is the remaining work, and
it is a small, well-posed question -- but it is in the SDK's CD path, not
in the video player.

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
