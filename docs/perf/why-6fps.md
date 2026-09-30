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
stops partway through a frame and the decode dies at about **slice 14 of
20**. Register-level trace of one frame, stream live versus stopped:

```
stream live                          stream stopped
col 11 pre=2E04038F inb=1            col 11 -> ...
col 12 pre=2E04038F inb=0   <- ch0 finally done
col 13 ok=1 post=9604FFFF   <- DATA_IN_FULL | DATA_OUT_EMPTY, dead
"MDEC produced no more pixels"
```

`0x9604FFFF` is the MDEC sitting with a full input buffer and an empty
output, having consumed only part of the frame. Ruled out along the way:
the ring contents are correct (`w0=38000D40`, the right decode command);
the IRQ mask is `0x1` (VBlank only, so no CD interrupt storm); masking the
idle CD DMA channel changes nothing; and the `stop()` in the shipped code
was load-bearing, not incidental.

Two workarounds were built and both made it worse:

- **Feed the MDEC through its data port** from the drain's service
  callback, removing the ch0 DMA entirely. The callback only fires every
  1024 spins, which cannot keep the input FIFO topped up: 150/150 frames
  failed, against 4/150 with the original path.
- **`pause_read`/`resume_read` around each decode** to keep the stream
  seek-free. `resume_read` re-delivers the sector that was in flight, so
  every frame is offset by a sector; with the discard it failed on all
  frames, without it on 4 then panicked. The SDK's own wording -- "the
  first sector *can* duplicate" -- is not something to build frame
  alignment on.

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
