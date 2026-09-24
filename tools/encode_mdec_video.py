#!/usr/bin/env python3
"""
Encodes MP4 video into PS1 MDEC hardware bitstream format at 320x240 @ 15 fps.
Produces fixed 16,384 bytes (8 sectors) per frame for seek-free disc streaming
and hardware MDEC decoding.
"""

import sys
import os
import math
import struct
import cv2
import numpy as np

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

S = [math.cos((i or 4) / 16 * math.pi) / 2 for i in range(8)]
DCT_MATRIX = np.array((
     S[0],  S[0],  S[0],  S[0],  S[0],  S[0],  S[0],  S[0],
     S[1],  S[3],  S[5],  S[7], -S[7], -S[5], -S[3], -S[1],
     S[2],  S[6], -S[6], -S[2], -S[2], -S[6],  S[6],  S[2],
     S[3], -S[7], -S[1], -S[5],  S[5],  S[1],  S[7], -S[3],
     S[4], -S[4], -S[4],  S[4],  S[4], -S[4], -S[4],  S[4],
     S[5], -S[1],  S[7],  S[3], -S[3], -S[7],  S[1], -S[5],
     S[6], -S[2],  S[2], -S[6], -S[6],  S[2], -S[2],  S[6],
     S[7], -S[5],  S[3], -S[1],  S[1], -S[3],  S[5], -S[7]
), np.float32).reshape((8, 8))

def to_int10(val):
    c = min(max(int(val), -0x200), 0x1ff)
    return c + (0 if c >= 0 else 0x400)

def encode_block(buf, blk, scale):
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
    y, cb, cr = blk.transpose((2, 0, 1))
    off = 0
    off += encode_block(buf[off:], cr[0:16:2, 0:16:2], cs)
    off += encode_block(buf[off:], cb[0:16:2, 0:16:2], cs)
    off += encode_block(buf[off:], y[0:8, 0:8], ys)
    off += encode_block(buf[off:], y[0:8, 8:16], ys)
    off += encode_block(buf[off:], y[8:16, 0:8], ys)
    off += encode_block(buf[off:], y[8:16, 8:16], ys)
    return off

def encode_frame(frame_bgr, luma_scale=16, chroma_scale=24):
    resized = cv2.resize(frame_bgr, (320, 240), interpolation=cv2.INTER_AREA)
    ycbcr = cv2.cvtColor(resized, cv2.COLOR_BGR2YCrCb)
    y_plane = ycbcr[:, :, 0]
    cr_plane = ycbcr[:, :, 1]
    cb_plane = ycbcr[:, :, 2]
    data = np.stack([y_plane, cb_plane, cr_plane], axis=2)

    buf = np.empty(0x80000, np.uint16)
    off = 0
    for x in range(0, 320, 16):
        for y in range(0, 240, 16):
            off += encode_macroblock(buf[off:], data[y:y+16, x:x+16], luma_scale, chroma_scale)

    # `off` counts 16-bit MDEC run/level values. The MDEC command length
    # and DMA transfer are measured in 32-bit words, so retain both
    # halfwords in every word instead of truncating the stream to half size.
    data_words = (off + 1) // 2
    return buf[:data_words * 2].tobytes(), data_words

def encode_frame_adaptive(frame_bgr, max_payload_bytes):
    for scale in range(16, 64, 2):
        chroma = min(63, int(scale * 1.25))
        rl_bytes, data_words = encode_frame(frame_bgr, luma_scale=scale, chroma_scale=chroma)
        if 4 + data_words * 4 <= max_payload_bytes:
            return rl_bytes, data_words, scale
    rl_bytes, data_words = encode_frame(frame_bgr, luma_scale=63, chroma_scale=63)
    return rl_bytes, data_words, 63

def main():
    in_video = "Videos/Platypus_Intro.mp4"
    out_vid = "Videos/INTRO.VID"
    out_embedded = "game/video_mdec.bin"

    cap = cv2.VideoCapture(in_video)
    if not cap.isOpened():
        print(f"Error: Could not open {in_video}")
        sys.exit(1)

    total_frames = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    target_frames = 150
    FRAME_WORDS = 4096 # 16,384 bytes = 8 sectors
    FRAME_BYTES = FRAME_WORDS * 4

    print(f"Encoding {in_video} ({total_frames} frames) to {out_vid} ({target_frames} frames @ 320x240, 15fps)...")

    full_data = bytearray()
    embedded_data = bytearray()
    max_words_used = 0

    for i in range(target_frames):
        src_frame_idx = int(round(i * (total_frames - 1) / (target_frames - 1)))
        cap.set(cv2.CAP_PROP_POS_FRAMES, src_frame_idx)
        ret, frame = cap.read()
        if not ret:
            print(f"Error reading frame {i}")
            break

        rl_bytes, data_words, used_scale = encode_frame_adaptive(frame, FRAME_BYTES)
        if data_words > max_words_used:
            max_words_used = data_words

        # Command word 0: 0x3800_0000 | data_words
        cmd_word = 0x38000000 | (data_words & 0xFFFF)
        frame_payload = struct.pack("<I", cmd_word) + rl_bytes

        assert len(frame_payload) <= FRAME_BYTES, f"Frame {i} too large: {len(frame_payload)}"
        frame_payload += b"\x00" * (FRAME_BYTES - len(frame_payload))

        full_data.extend(frame_payload)
        if i < 16:
            embedded_data.extend(frame_payload)

        if (i + 1) % 15 == 0 or i == target_frames - 1:
            print(f"  Frame {i+1:3d}/{target_frames} encoded (scale: {used_scale}, payload: {data_words*4+4} bytes / {FRAME_BYTES})")

    cap.release()

    with open(out_vid, "wb") as f:
        f.write(full_data)
    print(f"Wrote {len(full_data)} bytes to {out_vid} ({len(full_data)//2048} sectors)")

    with open(out_embedded, "wb") as f:
        f.write(embedded_data)
    print(f"Wrote {len(embedded_data)} bytes to {out_embedded} (16 frames)")

if __name__ == "__main__":
    main()
