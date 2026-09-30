#!/usr/bin/env python3
"""Host harness for the Plattypus intro-video performance bench.

Builds `video_bench` (a bare-metal PSX-EXE that plays the intro through the
game's own `video.rs` code with a stopwatch attached), masters a small disc
containing it plus `INTRO.VID`, runs that disc headless in an emulator, parses
the guest's TTY report, and writes `report.json` + a text summary.

Usage:
    tools/video_bench/host_bench.py [--runs N] [--emulator PATH] [--keep]

Emulator: DuckStation is used headless (`-nogui`). It logs BIOS TTY output to
its console, which is what the guest writes its report to. Any emulator that
forwards the expansion-port TTY to stdout works; set --emulator to override.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BENCH_DIR = REPO / "video_bench"
DIST = REPO / "dist"
MKISOPSX = REPO / "psoxide" / "tools" / "mkisopsx" / "Cargo.toml"

DEFAULT_EMULATORS = [
    Path.home() / "Downloads" / "DuckStation-x64.AppImage",
    Path("/usr/local/bin/duckstation"),
    Path("/usr/bin/duckstation"),
]

# DuckStation prefixes each log line with an emulated-time stamp and a level
# tag, and wraps the whole line in ANSI colour codes. Strip the codes first,
# then keep only the TTY payload the guest printed.
ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
TTY_RE = re.compile(r"I/TTY:\s?(.*)$")


def find_emulator(override: str | None) -> Path:
    if override:
        p = Path(override)
        if not p.exists():
            sys.exit(f"emulator not found: {p}")
        return p
    for cand in DEFAULT_EMULATORS:
        if cand.exists():
            return cand
    which = shutil.which("duckstation")
    if which:
        return Path(which)
    sys.exit(
        "no emulator found. Install DuckStation or pass --emulator PATH.\n"
        "The bench needs an emulator that logs BIOS TTY output to stdout."
    )


def run(cmd, cwd=None, timeout=900, capture=True):
    return subprocess.run(
        cmd, cwd=cwd, timeout=timeout, check=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None, text=True,
    )


def build_bench() -> Path:
    print("==> building video_bench (PSX-EXE)")
    run(["cargo", "build", "--release"], cwd=BENCH_DIR)
    exe = BENCH_DIR / "target" / "mipsel-sony-psx" / "release" / "video-bench.exe"
    if not exe.exists():
        sys.exit(f"build did not produce {exe}")
    return exe


def make_disc(exe: Path) -> Path:
    print("==> mastering bench disc")
    DIST.mkdir(parents=True, exist_ok=True)
    out = DIST / "videobench.bin"
    run([
        "cargo", "run", "--release", "--manifest-path", str(MKISOPSX), "--",
        "--exe", str(exe),
        "--out", str(out),
        "--volume", "VIDEOBENCH",
        "--str-file", str(REPO / "Videos" / "INTRO.VID"),
        "--str-file", str(REPO / "Videos" / "OUTRO.VID"),
    ])
    return DIST / "videobench.cue"


def run_disc(cue: Path, emulator: Path, seconds: int) -> str:
    """Run the disc headless and return the guest's TTY transcript.

    The emulator is captured to a file rather than a pipe: reading a pipe
    blocks whenever the emulator goes quiet, which would stop the deadline
    below from ever being checked. The guest prints a terminator line, so
    the common case still ends as soon as the report is complete.
    """
    print(f"==> running {cue.name} in {emulator.name} headless ({seconds}s budget)")
    env = dict(os.environ, APPIMAGE_EXTRACT_AND_RUN="1")
    log = Path("/tmp/videobench_emulator.log")
    cmd = ["timeout", "-s", "INT", "-k", "10", str(seconds),
           str(emulator), "-nogui", "-nofullscreen", str(cue)]
    with log.open("w") as fh:
        subprocess.run(
            cmd, stdout=fh, stderr=subprocess.STDOUT, text=True, env=env,
            timeout=seconds + 60, check=False,
        )
    raw = log.read_text(errors="replace")
    lines = []
    for l in raw.splitlines():
        m = TTY_RE.search(ANSI_RE.sub("", l))
        if m:
            lines.append(m.group(1))
    if not any(l.strip() == "@@VB1 END" for l in lines):
        # Say so plainly rather than letting the parser report a
        # confusing "never reached BEGIN".
        print(f"  NOTE: guest did not print @@VB1 END within {seconds}s "
              f"(emulator log: {log})", file=sys.stderr)
    return "\n".join(lines)


@dataclass
class Run:
    fields: dict = field(default_factory=dict)
    presents: list = field(default_factory=list)

    def get(self, group, key, default=0):
        return self.fields.get(f"{group}.{key}", default)


KV_RE = re.compile(r"@@VB1 (\S+) (\S+)=([0-9A-Fa-f]+)")
VBP_RE = re.compile(r"@@VBP ([0-9A-Fa-f]+) ([0-9A-Fa-f]+) ([0-9A-Fa-f]+) ([0-9A-Fa-f]+)")


def parse_tty(text: str) -> Run:
    run_ = Run()
    saw_begin = saw_end = False
    for line in text.splitlines():
        line = line.strip()
        if line == "@@VB1 BEGIN":
            saw_begin = True
        if line == "@@VB1 END":
            saw_end = True
        m = KV_RE.match(line)
        if m:
            group, key, val = m.groups()
            run_.fields[f"{group}.{key}"] = int(val, 16)
        m = VBP_RE.match(line)
        if m:
            idx, interval, work, read_vb = (int(x, 16) for x in m.groups())
            run_.presents.append(
                {"index": idx, "interval_vb": interval, "work_vb": work, "read_vb": read_vb}
            )
    if not saw_begin:
        raise RuntimeError("guest never reached @@VB1 BEGIN (did it boot?)")
    if not saw_end:
        raise RuntimeError("guest did not finish its report (@@VB1 END missing)")
    return run_


def summarise(r: Run) -> dict:
    f = r.fields
    intervals = [p["interval_vb"] for p in r.presents]
    works = [p["work_vb"] for p in r.presents]
    dist: dict = {}
    for i in intervals:
        dist[i] = dist.get(i, 0) + 1
    return {
        "clock": "cycles" if f.get("probe.clock", 0) == 1 else "vblank",
        "rcnt_live": bool(f.get("probe.rcnt_live", 0)),
        "overflow_live": bool(f.get("probe.overflow_live", 0)),
        "using_cd": bool(f.get("env.using_cd", 0)),
        "located": bool(f.get("env.located", 0)),
        "integrity_ok": bool(f.get("integ.ok", 0)),
        "presented": f.get("paced.presented", 0),
        "fps": f.get("paced.fps_x1000", 0) / 1000.0,
        "interval_min_vb": f.get("paced.interval_min", 0),
        "interval_mean_vb": f.get("paced.interval_mean", 0),
        "interval_p95_vb": f.get("paced.interval_p95", 0),
        "interval_max_vb": f.get("paced.interval_max", 0),
        "stutters": f.get("paced.stutters", 0),
        "avg_frame_time_us": f.get("paced.avg_frame_time_us", 0),
        "work_overruns": f.get("paced.work_overruns", 0),
        "work_vb_max": f.get("paced.work_vb_max", 0),
        "read_us_per_frame": f.get("burst.read_us", 0),
        "copy_us_per_frame": f.get("burst.copy_us", 0),
        "decode_us_per_frame": f.get("burst.decode_us", 0),
        "upload_us_per_frame": f.get("burst.upload_us", 0),
        "upload_gp0_us_per_frame": f.get("burst.upload_gp0_us", 0),
        "vram_dma_fallbacks": f.get("burst.vram_dma_fallbacks", 0),
        "pipeline_us_per_frame": f.get("burst.pipeline_us", 0),
        "budget_pct": f.get("burst.budget_pct", 0),
        "read_pct": f.get("burst.read_pct", 0),
        "copy_pct": f.get("burst.copy_pct", 0),
        "decode_pct": f.get("burst.decode_pct", 0),
        "upload_pct": f.get("burst.upload_pct", 0),
        "real_us_per_frame": f.get("real.total_us", 0),
        "closure_gap_pct": f.get("real.closure_gap_pct", 0),
        "cd_contiguous_sectors": f.get("cdrate.contiguous_sectors", 0),
        "cd_contiguous_vblanks": f.get("cdrate.contiguous_vblanks", 0),
        "cd_batched_sectors": f.get("cdrate.batched_sectors", 0),
        "cd_batched_vblanks": f.get("cdrate.batched_vblanks", 0),
        "chunk_starts": f.get("paced_only.chunk_starts", 0),
        "overlapped_sectors": f.get("paced_only.overlapped_sectors", 0),
        "interval_histogram_vb": dist,
        "per_present": r.presents,
    }


def print_report(s: dict):
    def line(label, value, note=""):
        print(f"  {label:<26} {value:>12}  {note}")

    print()
    print("  INTRO VIDEO PERFORMANCE")
    print("  " + "-" * 62)
    print(f"  {'clock':<26} {s['clock']:>12}  "
          f"{'(RCNT+overflow)' if s['clock']=='cycles' else '(display periods)'}")
    line("video source", "CD" if s["using_cd"] else "EMBEDDED",
         "INTRO.VID found" if s["located"] else "NOT FOUND ON DISC")
    line("decode integrity", "OK" if s["integrity_ok"] else "FAIL")
    print("  " + "-" * 62)
    print("  PACED PLAYBACK (what the viewer sees)")
    line("frames presented", s["presented"])
    line("presented fps", f"{s['fps']:.2f}", "(target 15.00)")
    line("frame interval min", f"{s['interval_min_vb']} vb", "(target 4)")
    line("frame interval mean", f"{s['interval_mean_vb']} vb")
    line("frame interval p95", f"{s['interval_p95_vb']} vb")
    line("frame interval max", f"{s['interval_max_vb']} vb")
    line("avg frame time", f"{s['avg_frame_time_us']/1000:.2f} ms", "(target 66.67)")
    line("stuttering presents", s["stutters"], f"of {s['presented']}")
    line("draw() overruns", s["work_overruns"], f"(worst {s['work_vb_max']} vb)")
    if "chunk_starts" in s:
        line("chunk starts (seeks)", s["chunk_starts"], "stop+re-seek cycles")
        line("sectors read under a decode", s["overlapped_sectors"], "of 1050 total")
    print("  " + "-" * 62)
    print("  STAGE COST (burst, per frame)")
    line("CD read", f"{s['read_us_per_frame']/1000:.2f} ms", f"{s['read_pct']}% of budget")
    line("cache->work copy", f"{s['copy_us_per_frame']/1000:.2f} ms", f"{s['copy_pct']}%")
    line("MDEC decode", f"{s['decode_us_per_frame']/1000:.2f} ms", f"{s['decode_pct']}%")
    line("VRAM upload", f"{s['upload_us_per_frame']/1000:.2f} ms", f"{s['upload_pct']}%")
    line("pipeline total", f"{s['pipeline_us_per_frame']/1000:.2f} ms", f"{s['budget_pct']}% of one 16.7 ms display period")
    line("shipped draw() total", f"{s['real_us_per_frame']/1000:.2f} ms",
         f"closure gap {s['closure_gap_pct']}%")
    print("  " + "-" * 62)
    print("  CD THROUGHPUT")
    # 75 sectors/s is 1x, 150 is 2x. Derived from the raw counts the guest
    # reports rather than a rate it would round to whole sectors.
    def rate(sect, vb):
        return (sect * 60.0 / vb) if vb else 0.0
    cont = rate(s["cd_contiguous_sectors"], s["cd_contiguous_vblanks"])
    batch = rate(s["cd_batched_sectors"], s["cd_batched_vblanks"])
    line("contiguous (1 seek)", f"{cont:.0f} sect/s",
         f"{s['cd_contiguous_sectors']} sectors in {s['cd_contiguous_vblanks']} vb; "
         "75=1x, 150=2x")
    line("shipped (re-seek/batch)", f"{batch:.0f} sect/s",
         f"{s['cd_batched_sectors']} sectors in {s['cd_batched_vblanks']} vb")
    if cont and batch:
        ratio = cont / batch
        line("contiguous / batched", f"{ratio:.2f}x",
             "seek dominates" if ratio > 1.3 else "drive-limited, not seek-bound")
    print("  " + "-" * 62)
    if s["interval_histogram_vb"]:
        print("  present-interval histogram (display periods -> count)")
        for k in sorted(s["interval_histogram_vb"]):
            bar = "#" * min(s["interval_histogram_vb"][k], 60)
            print(f"    {k:>3} vb  {s['interval_histogram_vb'][k]:>4}  {bar}")
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=1, help="repeat the measurement N times")
    ap.add_argument("--emulator", default=None)
    ap.add_argument("--seconds", type=int, default=120, help="wall-clock budget per run")
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--out", default=str(DIST / "video_bench_report.json"))
    args = ap.parse_args()

    emulator = find_emulator(args.emulator)
    if args.no_build:
        exe = BENCH_DIR / "target" / "mipsel-sony-psx" / "release" / "video-bench.exe"
        if not exe.exists():
            sys.exit(f"--no-build given but {exe} does not exist")
    else:
        exe = build_bench()
    cue = make_disc(exe)

    results = []
    for i in range(args.runs):
        print(f"--> run {i+1}/{args.runs}")
        tty = run_disc(cue, emulator, args.seconds)
        try:
            r = parse_tty(tty)
        except RuntimeError as e:
            print(f"  ERROR: {e}", file=sys.stderr)
            Path("/tmp/videobench_tty.log").write_text(tty)
            print("  guest TTY saved to /tmp/videobench_tty.log", file=sys.stderr)
            sys.exit(2)
        s = summarise(r)
        results.append(s)
        print_report(s)

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "emulator": str(emulator),
        "runs": results,
        "median": median_summary(results),
    }
    out.write_text(json.dumps(payload, indent=2))
    print(f"wrote {out}")

    m = payload["median"]
    print()
    print(f"MEDIAN over {args.runs} run(s): {m['fps']:.2f} fps, "
          f"{m['stutters']}/{m['presented']} stuttering presents, "
          f"pipeline {m['pipeline_us_per_frame']/1000:.2f} ms/frame "
          f"({m['budget_pct']}% of budget)")


def median_summary(results: list[dict]) -> dict:
    import statistics
    if len(results) == 1:
        return results[0]
    keys = [k for k, v in results[0].items() if isinstance(v, (int, float)) and not isinstance(v, bool)]
    med = dict(results[0])
    for k in keys:
        med[k] = statistics.median(r[k] for r in results)
    return med


if __name__ == "__main__":
    main()
