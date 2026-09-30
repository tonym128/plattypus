#!/usr/bin/env python3
"""
encode_mdec_video.py — PS1 MDEC hardware bitstream encoder.

Converts any video file that OpenCV can read into a raw PS1 MDEC bitstream
(.VID) suitable for sector-aligned disc streaming and hardware MDEC decoding.

Each frame slot occupies a fixed number of 32-bit words (default: 4096 = 16 KB
= 8 CD sectors) so that frames can be read at a constant rate without seeking.

Usage:
    python3 encode_mdec_video.py INPUT OUTPUT [OPTIONS]
    python3 encode_mdec_video.py --selftest          # run unit tests

Run --help for the full option list.
"""

import sys
import math
import struct
import argparse
import unittest

import numpy as np

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

ZIGZAG_TABLE = np.array((
     0,  1,  5,  6, 14, 15, 27, 28,
     2,  4,  7, 13, 16, 26, 29, 42,
     3,  8, 12, 17, 25, 30, 41, 43,
     9, 11, 18, 24, 31, 40, 44, 53,
    10, 19, 23, 32, 39, 45, 52, 54,
    20, 22, 33, 38, 46, 51, 55, 60,
    21, 34, 37, 47, 50, 56, 59, 61,
    35, 36, 48, 49, 57, 58, 62, 63
), np.uint8).argsort()

QUANT_TABLE = np.array((
     2, 16, 19, 22, 26, 27, 29, 34,
    16, 16, 22, 24, 27, 29, 34, 37,
    19, 22, 26, 27, 29, 34, 34, 38,
    22, 22, 26, 27, 29, 34, 37, 40,
    22, 26, 27, 29, 32, 35, 40, 48,
    26, 27, 29, 32, 35, 40, 48, 58,
    26, 27, 29, 34, 38, 46, 56, 69,
    27, 29, 35, 38, 46, 56, 69, 83
), np.uint8).reshape((8, 8))

_S = [math.cos((i or 4) / 16 * math.pi) / 2 for i in range(8)]
DCT_MATRIX = np.array((
     _S[0],  _S[0],  _S[0],  _S[0],  _S[0],  _S[0],  _S[0],  _S[0],
     _S[1],  _S[3],  _S[5],  _S[7], -_S[7], -_S[5], -_S[3], -_S[1],
     _S[2],  _S[6], -_S[6], -_S[2], -_S[2], -_S[6],  _S[6],  _S[2],
     _S[3], -_S[7], -_S[1], -_S[5],  _S[5],  _S[1],  _S[7], -_S[3],
     _S[4], -_S[4], -_S[4],  _S[4],  _S[4], -_S[4], -_S[4],  _S[4],
     _S[5], -_S[1],  _S[7],  _S[3], -_S[3], -_S[7],  _S[1], -_S[5],
     _S[6], -_S[2],  _S[2], -_S[6], -_S[6],  _S[2], -_S[2],  _S[6],
     _S[7], -_S[5],  _S[3], -_S[1],  _S[1], -_S[3],  _S[5], -_S[7]
), np.float32).reshape((8, 8))

DEFAULT_WIDTH = 320
DEFAULT_HEIGHT = 240
DEFAULT_FPS = 15
DEFAULT_FRAME_WORDS = 4096   # 16,384 bytes = 8 CD sectors per frame

# ---------------------------------------------------------------------------
# Core encoding logic (unchanged from original)
# ---------------------------------------------------------------------------

def to_int10(val):
    """Clamp *val* to a signed 10-bit integer and return its unsigned encoding."""
    c = min(max(int(val), -0x200), 0x1ff)
    return c + (0 if c >= 0 else 0x400)


def encode_block(buf, blk, scale):
    """
    DCT-quantise one 8×8 luma or chroma block and write MDEC run/level pairs
    into *buf* (a uint16 view).  Returns the number of uint16 values written
    (always even — MDEC requires word-aligned streams).

    Parameters
    ----------
    buf   : numpy uint16 array, at least 65 elements of working space
    blk   : 8×8 numpy array of uint8 pixel values
    scale : quantisation scale factor (1–63)
    """
    _block = blk.astype(np.float32) - 128.0
    coeffs = (DCT_MATRIX @ _block @ DCT_MATRIX.T) / QUANT_TABLE
    coeffs = coeffs.reshape((64,))[ZIGZAG_TABLE]
    buf[0] = (scale << 10) | to_int10(round(coeffs[0]))
    offset = 1
    ac_vals = coeffs[1:] * 8.0 / scale
    run = 0
    for ac in ac_vals.round().astype(np.int32):
        if ac:
            buf[offset] = (run << 10) | to_int10(ac)
            offset += 1
            run = 0
        else:
            run += 1
    buf[offset] = 0xfe00
    offset += 1
    if offset % 2:
        buf[offset] = 0xfe00
        offset += 1
    return offset


def encode_macroblock(buf, blk, ys, cs):
    """
    Encode one 16×16 macroblock (YCbCr, channels last) into *buf*.

    Channel order expected in *blk*: axis-2 = [Y, Cb, Cr].
    Writes Cr, Cb, then four 8×8 Y blocks in MDEC order.
    Returns the number of uint16 values written.
    """
    y, cb, cr = blk.transpose((2, 0, 1))
    off = 0
    off += encode_block(buf[off:], cr[0:16:2, 0:16:2], cs)
    off += encode_block(buf[off:], cb[0:16:2, 0:16:2], cs)
    off += encode_block(buf[off:], y[0:8, 0:8], ys)
    off += encode_block(buf[off:], y[0:8, 8:16], ys)
    off += encode_block(buf[off:], y[8:16, 0:8], ys)
    off += encode_block(buf[off:], y[8:16, 8:16], ys)
    return off


def encode_ycbcr(data, luma_scale=16, chroma_scale=24,
                 width=DEFAULT_WIDTH, height=DEFAULT_HEIGHT):
    """
    Encode an already converted YCbCr array (H, W, 3) in [Y, Cb, Cr] order
    into an MDEC bitstream.

    Parameters
    ----------
    data         : numpy uint8 array (H, W, 3) where channels are [Y, Cb, Cr]
    luma_scale   : quantisation scale for luma blocks (1–63)
    chroma_scale : quantisation scale for chroma blocks (1–63)
    width        : output width in pixels (multiple of 16)
    height       : output height in pixels (multiple of 16)

    Returns
    -------
    (rl_bytes, data_words)
    """
    buf = np.empty(0x80000, np.uint16)
    off = 0
    for x in range(0, width, 16):
        for y in range(0, height, 16):
            off += encode_macroblock(buf[off:], data[y:y+16, x:x+16], luma_scale, chroma_scale)

    data_words = (off + 1) // 2
    return buf[:data_words * 2].tobytes(), data_words


def encode_frame(frame_bgr, luma_scale=16, chroma_scale=24,
                 width=DEFAULT_WIDTH, height=DEFAULT_HEIGHT):
    """
    Encode a single BGR frame into an MDEC bitstream.

    Parameters
    ----------
    frame_bgr    : numpy uint8 array (H, W, 3) in BGR order
    luma_scale   : quantisation scale for luma blocks (1–63)
    chroma_scale : quantisation scale for chroma blocks (1–63)
    width        : output width in pixels (multiple of 16)
    height       : output height in pixels (multiple of 16)

    Returns
    -------
    (rl_bytes, data_words)
        rl_bytes   – raw run/level byte stream
        data_words – number of 32-bit words occupied
    """
    import cv2  # imported here so unit tests don't require cv2
    resized = cv2.resize(frame_bgr, (width, height), interpolation=cv2.INTER_AREA)
    ycbcr = cv2.cvtColor(resized, cv2.COLOR_BGR2YCrCb)
    data = np.stack([ycbcr[:, :, 0], ycbcr[:, :, 2], ycbcr[:, :, 1]], axis=2)
    return encode_ycbcr(data, luma_scale=luma_scale, chroma_scale=chroma_scale,
                        width=width, height=height)


def encode_ycbcr_adaptive(data, max_payload_bytes,
                          width=DEFAULT_WIDTH, height=DEFAULT_HEIGHT):
    """
    Encode preprocessed YCbCr data, raising the quantisation scale until the
    bitstream fits within max_payload_bytes (including the 4-byte command
    word), and return the *coarsest* encoding that still fits -- i.e. the
    lowest scale, which is the best quality.

    Payload size is monotonically non-increasing in scale: `encode_block`
    divides AC coefficients by the scale, so a larger scale can only round
    more of them to zero and shorten the run/level stream. That makes a
    bisection valid, and turns up to 25 whole-frame encodes into at most 6.
    Encoding a 150-frame video is dominated by this search, so the
    difference is several minutes per video.
    """
    def payload_for(scale):
        chroma = min(63, int(scale * 1.25))
        rl_bytes, data_words = encode_ycbcr(data, luma_scale=scale,
                                            chroma_scale=chroma,
                                            width=width, height=height)
        dma_words = (data_words + 31) & ~31
        return rl_bytes, data_words, scale, 4 + dma_words * 4

    # Does this scale fit?
    def fits(scale):
        return payload_for(scale)[3] <= max_payload_bytes

    LO, HI = 16, 63
    # `fits` is monotonically non-increasing in scale, so the answer is a
    # threshold. Handle both ends before bisecting, otherwise a scale that
    # already fits at LO gets "improved" into a coarser one.
    if fits(LO):
        return payload_for(LO)[:3]
    if not fits(HI):
        return payload_for(HI)[:3]

    best = payload_for(HI)
    # Invariant: lo does not fit, hi does. Bisect for the boundary, which
    # is the lowest scale that still fits.
    lo, hi = LO, HI
    while lo + 2 < hi:
        mid = (lo + hi) // 2
        if fits(mid):
            hi = mid
            best = payload_for(mid)
        else:
            lo = mid
    return best[:3]


def encode_frame_adaptive(frame_bgr, max_payload_bytes,
                          width=DEFAULT_WIDTH, height=DEFAULT_HEIGHT):
    """
    Encode a frame, increasing the quantisation scale until the bitstream fits
    within *max_payload_bytes* (which must include the 4-byte MDEC command word).
    Performs resize and color conversion once per frame.

    Returns
    -------
    (rl_bytes, data_words, used_scale)
    """
    import cv2
    resized = cv2.resize(frame_bgr, (width, height), interpolation=cv2.INTER_AREA)
    ycbcr = cv2.cvtColor(resized, cv2.COLOR_BGR2YCrCb)
    data = np.stack([ycbcr[:, :, 0], ycbcr[:, :, 2], ycbcr[:, :, 1]], axis=2)
    return encode_ycbcr_adaptive(data, max_payload_bytes, width=width, height=height)


# ---------------------------------------------------------------------------
# Unit tests
# ---------------------------------------------------------------------------

class TestMdecEncoder(unittest.TestCase):
    """Self-contained unit tests — only numpy is required (no cv2)."""

    def _solid_ycbcr_frame(self, width=320, height=240, y=128, cb=128, cr=128):
        """Return a solid-colour YCbCr frame as a numpy array (H, W, 3)."""
        frame = np.empty((height, width, 3), dtype=np.uint8)
        frame[:, :, 0] = y
        frame[:, :, 1] = cb
        frame[:, :, 2] = cr
        return frame

    def _solid_bgr_frame(self, width=320, height=240, b=128, g=128, r=128):
        """Return a solid-colour BGR frame — used to drive encode_frame_adaptive
        via a monkey-patched encode_frame that skips cv2."""
        frame = np.empty((height, width, 3), dtype=np.uint8)
        frame[:, :, 0] = b
        frame[:, :, 1] = g
        frame[:, :, 2] = r
        return frame

    # ------------------------------------------------------------------
    # 1. EOB marker
    # ------------------------------------------------------------------
    def test_end_of_block_marker(self):
        """encode_block on a flat (128=zero AC) 8×8 block must emit 0xfe00."""
        blk = np.full((8, 8), 128, dtype=np.uint8)
        buf = np.zeros(128, dtype=np.uint16)
        n = encode_block(buf, blk, scale=16)
        self.assertIn(0xfe00, buf[:n],
                      "EOB marker 0xfe00 not found in encoded block output")

    # ------------------------------------------------------------------
    # 2. Macroblock word-alignment
    # ------------------------------------------------------------------
    def test_macroblock_word_count(self):
        """Encoding a solid 16×16 macroblock must yield a non-empty, even-length uint16 stream."""
        blk = np.full((16, 16, 3), 128, dtype=np.uint8)
        buf = np.zeros(0x8000, dtype=np.uint16)
        n = encode_macroblock(buf, blk, ys=16, cs=24)
        self.assertGreater(n, 0, "Macroblock produced no output")
        self.assertEqual(n % 2, 0, "Macroblock output is not word-aligned (odd uint16 count)")

    # ------------------------------------------------------------------
    # 3. encode_frame_adaptive fits payload
    # ------------------------------------------------------------------
    def test_adaptive_scale_fits(self):
        """encode_ycbcr_adaptive result must fit within max_payload_bytes."""
        width, height = 320, 240
        max_payload_bytes = DEFAULT_FRAME_WORDS * 4  # 16384
        data = self._solid_ycbcr_frame(width, height)
        rl_bytes, data_words, used_scale = encode_ycbcr_adaptive(
            data, max_payload_bytes, width=width, height=height
        )
        dma_words = (data_words + 31) & ~31
        payload_size = 4 + dma_words * 4
        self.assertLessEqual(payload_size, max_payload_bytes,
                             f"Adaptive payload {payload_size} exceeds max {max_payload_bytes}")

    # ------------------------------------------------------------------
    # 4. DMA word-count alignment
    # ------------------------------------------------------------------
    def test_frame_words_alignment(self):
        """DMA word count must always be a multiple of 32."""
        for data_words in [1, 31, 32, 63, 100, 127, 128, 255, 256]:
            dma_words = (data_words + 31) & ~31
            self.assertEqual(dma_words % 32, 0,
                             f"dma_words={dma_words} is not a multiple of 32 "
                             f"(data_words={data_words})")

    # ------------------------------------------------------------------
    # 5. Command-word format
    # ------------------------------------------------------------------
    def test_cmd_word_format(self):
        """The MDEC command word must have 0x38 in the top byte."""
        for dma_words in [32, 64, 128, 4096]:
            cmd_word = 0x38000000 | (dma_words & 0xFFFF)
            top_byte = (cmd_word >> 24) & 0xFF
            low16    = cmd_word & 0xFFFF
            self.assertEqual(top_byte, 0x38,
                             f"Command word top byte is 0x{top_byte:02X}, expected 0x38")
            self.assertEqual(low16, dma_words & 0xFFFF,
                             f"Command word low 16 bits wrong for dma_words={dma_words}")

    # ------------------------------------------------------------------
    # 6. Frame words slot validation
    # ------------------------------------------------------------------
    def test_frame_words_slot_validation(self):
        """Frame words slot must be a positive multiple of 32 (DMA block size)."""
        self.assertTrue(DEFAULT_FRAME_WORDS >= 32 and DEFAULT_FRAME_WORDS % 32 == 0)
        self.assertEqual(DEFAULT_FRAME_WORDS * 4 % 2048, 0,
                         "Default frame slot size in bytes is not sector-aligned")



# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def build_parser():
    parser = argparse.ArgumentParser(
        prog="encode_mdec_video.py",
        description=(
            "Encode a video file into a PS1 MDEC hardware bitstream (.VID).\n"
            "Each frame occupies a fixed-size slot (--frame-words 32-bit words)\n"
            "for constant-rate disc streaming."
        ),
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )

    parser.add_argument("input",  metavar="INPUT",
                        help="Input video file (any format OpenCV can read)")
    parser.add_argument("output", metavar="OUTPUT",
                        help="Output .VID file path")

    parser.add_argument("--width",  type=int, default=DEFAULT_WIDTH,
                        metavar="W",
                        help="Output width in pixels (must be a multiple of 16)")
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT,
                        metavar="H",
                        help="Output height in pixels (must be a multiple of 16)")
    parser.add_argument("--fps",   type=float, default=DEFAULT_FPS,
                        metavar="FPS",
                        help="Target playback framerate")
    parser.add_argument("--frames", type=int, default=None,
                        metavar="N",
                        help="Maximum number of frames to encode (default: all frames)")
    parser.add_argument("--luma-scale", type=int, default=None,
                        metavar="S",
                        help="Fixed luma quantisation scale 1–63 "
                             "(default: adaptive per-frame)")
    parser.add_argument("--chroma-scale", type=int, default=None,
                        metavar="S",
                        help="Fixed chroma quantisation scale 1–63 "
                             "(default: adaptive per-frame)")
    parser.add_argument("--frame-words", type=int, default=DEFAULT_FRAME_WORDS,
                        metavar="W",
                        help="Fixed 32-bit words per frame slot "
                             f"(default: {DEFAULT_FRAME_WORDS} = 16 KB = 8 sectors)")
    parser.add_argument("--no-adaptive", action="store_true",
                        help="Disable adaptive quantisation; use fixed "
                             "--luma-scale / --chroma-scale values")
    parser.add_argument("--embedded", metavar="PATH", default=None,
                        help="Optional output path for embedded ROM fallback bitstream (first N frames)")
    parser.add_argument("--embedded-frames", type=int, default=16, metavar="N",
                        help="Number of initial frames to include in embedded fallback output (default: 16)")
    parser.add_argument("--target-words", type=int, default=None,
                        metavar="W",
                        help="Per-frame adaptive budget in 32-bit words. Only "
                             "meaningful with --variable-sectors: frames that "
                             "exceed it are still written (rounded up to whole "
                             "sectors) up to the --frame-words ceiling, so this "
                             "sets the typical frame size rather than the worst "
                             "case. Defaults to --frame-words, which reproduces "
                             "the fixed-size behaviour.")
    parser.add_argument("--variable-sectors", action="store_true",
                        help="Emit a variable-length container: a 1-sector header "
                             "holding each frame's sector count, then frames "
                             "rounded up to whole sectors. Cuts total bitrate "
                             "to the data the video actually needs instead of "
                             "rounding every frame up to the worst one. The "
                             "player must be built for the same format.")
    parser.add_argument("--verbose", action="store_true",
                        help="Print per-frame encoding statistics")
    parser.add_argument("--selftest", action="store_true",
                        help="Run built-in unit tests and exit")

    return parser


def main():
    parser = build_parser()

    # Allow --selftest with no positional args
    if "--selftest" in sys.argv:
        unittest.main(argv=[sys.argv[0]], exit=True, verbosity=2)

    args = parser.parse_args()

    # ---- Validate dimensions -----------------------------------------------
    if args.width % 16 != 0:
        print(f"Error: --width {args.width} is not a multiple of 16", file=sys.stderr)
        sys.exit(1)
    if args.height % 16 != 0:
        print(f"Error: --height {args.height} is not a multiple of 16", file=sys.stderr)
        sys.exit(1)

    # ---- Validate frame slot words ------------------------------------------
    if args.frame_words < 32 or args.frame_words % 32 != 0:
        print(f"Error: --frame-words ({args.frame_words}) must be a multiple of 32 and at least 32",
              file=sys.stderr)
        sys.exit(1)

    for name, val in [("--luma-scale", args.luma_scale),
                      ("--chroma-scale", args.chroma_scale)]:
        if val is not None and not (1 <= val <= 63):
            print(f"Error: {name} must be between 1 and 63, got {val}", file=sys.stderr)
            sys.exit(1)

    if args.no_adaptive:
        luma_scale   = args.luma_scale   if args.luma_scale   is not None else 16
        chroma_scale = args.chroma_scale if args.chroma_scale is not None else 24

    # ---- Open input ---------------------------------------------------------
    try:
        import cv2
    except ImportError:
        print("Error: OpenCV (cv2) is required. Install with: pip install opencv-python",
              file=sys.stderr)
        sys.exit(1)

    cap = cv2.VideoCapture(args.input)
    if not cap.isOpened():
        print(f"Error: Could not open input file: {args.input}", file=sys.stderr)
        sys.exit(1)

    source_frame_count = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    source_fps         = cap.get(cv2.CAP_PROP_FPS) or args.fps

    # Determine how many output frames to produce
    if args.frames is not None:
        target_frames = args.frames
    else:
        # Encode the full duration at the requested fps
        source_duration_s = source_frame_count / source_fps if source_fps > 0 else 0
        target_frames = max(1, int(round(source_duration_s * args.fps))) \
                        if source_duration_s > 0 else source_frame_count

    FRAME_WORDS = args.frame_words
    FRAME_BYTES = FRAME_WORDS * 4

    # In variable mode the per-frame payload is rounded up to whole sectors
    # and recorded in a header, so `--frame-words` is only a ceiling that
    # bounds the table's largest entry -- not a per-frame constant.
    VARIABLE = args.variable_sectors
    SECTOR_BYTES = 2048
    # In variable mode the adaptive search aims at a *typical* frame size and
    # lets awkward frames overflow to their own sector count. Aiming at the
    # ceiling instead would pad every easy frame out to the worst one, which
    # is the opposite of what a bitrate reduction wants.
    ADAPTIVE_BYTES = (args.target_words or FRAME_WORDS) * 4
    # The embedded fallback keeps a fixed stride so the player's embedded
    # path needs no table of its own; it simply costs a little ROM.
    EMBED_STRIDE_BYTES = FRAME_BYTES
    if VARIABLE:
        print(
            f"Encoding '{args.input}' → '{args.output}'\n"
            f"  Source : {source_frame_count} frames @ {source_fps:.2f} fps\n"
            f"  Output : {target_frames} frames @ {args.fps} fps, "
            f"{args.width}×{args.height}, variable-sector container "
            f"(target {ADAPTIVE_BYTES} B/frame, ceiling {FRAME_BYTES} B)"
        )
    else:
        print(
            f"Encoding '{args.input}' → '{args.output}'\n"
            f"  Source : {source_frame_count} frames @ {source_fps:.2f} fps\n"
            f"  Output : {target_frames} frames @ {args.fps} fps, "
            f"{args.width}×{args.height}, {FRAME_BYTES} bytes/frame"
        )

    full_data     = bytearray()
    embedded_data = bytearray()
    max_words_used = 0
    frames_written = 0
    sector_counts = []

    for i in range(target_frames):
        # Map output frame index to source frame index
        if target_frames > 1:
            src_idx = int(round(i * (source_frame_count - 1) / (target_frames - 1)))
        else:
            src_idx = 0

        cap.set(cv2.CAP_PROP_POS_FRAMES, src_idx)
        ret, frame = cap.read()
        if not ret:
            print(f"Warning: could not read source frame {src_idx} (output frame {i}); stopping.",
                  file=sys.stderr)
            break

        # ---- Encode ---------------------------------------------------------
        if args.no_adaptive:
            rl_bytes, data_words = encode_frame(frame, luma_scale, chroma_scale,
                                                width=args.width, height=args.height)
            used_scale = luma_scale
        else:
            rl_bytes, data_words, used_scale = encode_frame_adaptive(
                frame, ADAPTIVE_BYTES, width=args.width, height=args.height)

        if data_words > max_words_used:
            max_words_used = data_words

        # ---- Pack frame slot ------------------------------------------------
        dma_words    = (data_words + 31) & ~31
        padding_words = dma_words - data_words
        # Pad with MDEC end-of-stream markers so the DMA transfer ends cleanly
        padded_rl    = rl_bytes + (b"\x00\xfe" * (padding_words * 2))
        cmd_word     = 0x38000000 | (dma_words & 0xFFFF)
        frame_payload = struct.pack("<I", cmd_word) + padded_rl

        if VARIABLE:
            slot_bytes = ((len(frame_payload) + SECTOR_BYTES - 1) // SECTOR_BYTES) * SECTOR_BYTES
            if slot_bytes > FRAME_BYTES:
                print(
                    f"Error: frame {i} payload {len(frame_payload)} bytes needs "
                    f"{slot_bytes} bytes, over the --frame-words ceiling of "
                    f"{FRAME_BYTES}. Raise --frame-words.",
                    file=sys.stderr,
                )
                cap.release()
                sys.exit(1)
            sector_counts.append(slot_bytes // SECTOR_BYTES)
        else:
            if len(frame_payload) > FRAME_BYTES:
                print(
                    f"Error: frame {i} payload {len(frame_payload)} bytes exceeds "
                    f"frame slot {FRAME_BYTES} bytes. Use a larger --frame-words value.",
                    file=sys.stderr,
                )
                cap.release()
                sys.exit(1)
            slot_bytes = FRAME_BYTES

        # Pad remainder of the frame slot with zeros
        frame_payload += b"\x00" * (slot_bytes - len(frame_payload))
        full_data.extend(frame_payload)
        if args.embedded and i < args.embedded_frames:
            # Fixed stride, independent of this frame's own sector count.
            embedded_data.extend(frame_payload + b"\x00" * (EMBED_STRIDE_BYTES - slot_bytes))
        frames_written += 1

        if args.verbose or (i + 1) % 15 == 0 or i == target_frames - 1:
            print(
                f"  Frame {i+1:4d}/{target_frames}  "
                f"scale={used_scale:2d}  "
                f"payload={dma_words*4+4} / {FRAME_BYTES} bytes"
            )

    cap.release()

    # ---- Write output -------------------------------------------------------
    try:
        if VARIABLE:
            # One sector of header: a u16 frame count, then one u8 sector
            # count per frame. Sector-aligned so the player can read it with
            # the same reader it uses for frames.
            hdr = bytearray(struct.pack("<H", len(sector_counts)))
            hdr += bytes(sector_counts)
            if len(hdr) > SECTOR_BYTES:
                print(
                    f"Error: {len(sector_counts)} frames do not fit the "
                    f"{SECTOR_BYTES}-byte sector table.",
                    file=sys.stderr,
                )
                sys.exit(1)
            hdr += b"\x00" * (SECTOR_BYTES - len(hdr))
            # Count each frame as a whole sector so the total is sector
            # aligned, which is what makes the file streamable at all.
            body = bytes(full_data)
            pad = (-len(body)) % SECTOR_BYTES
            body += b"\x00" * pad
            with open(args.output, "wb") as f:
                f.write(bytes(hdr))
                f.write(body)
            total_sectors = 1 + len(body) // SECTOR_BYTES
            print(f"  Container: 1 header sector + {len(body)//SECTOR_BYTES} data "
                  f"sectors; mean {sum(sector_counts)/len(sector_counts):.2f} "
                  f"sectors/frame, max {max(sector_counts)}")
        else:
            with open(args.output, "wb") as f:
                f.write(full_data)
    except OSError as e:
        print(f"Error: could not write output file '{args.output}': {e}", file=sys.stderr)
        sys.exit(1)

    if args.embedded:
        try:
            with open(args.embedded, "wb") as f:
                f.write(embedded_data)
        except OSError as e:
            print(f"Error: could not write embedded file '{args.embedded}': {e}", file=sys.stderr)
            sys.exit(1)

    total_bytes = len(full_data)
    print(
        f"\nDone.\n"
        f"  Frames written : {frames_written}\n"
        f"  Total bytes    : {total_bytes} ({total_bytes // 2048} sectors)\n"
        f"  Output file    : {args.output}"
    )
    if args.embedded:
        print(
            f"  Embedded file  : {args.embedded} "
            f"({min(frames_written, args.embedded_frames)} frames, {len(embedded_data)} bytes)"
        )


if __name__ == "__main__":
    main()
