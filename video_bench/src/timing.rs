//! Timing primitives for the intro-video benchmark.
//!
//! # Why the clock is chosen at run time
//!
//! Every headline metric (presented frame rate, present-to-present
//! interval, stutter count) is derived from the VBlank interrupt counter
//! and nothing else. That counter is driven by the display model, which
//! every emulator and real silicon gets right, it cannot wrap or alias,
//! and one unit is exactly one thing the viewer cares about: one display
//! period. So the headline numbers are the same wherever this runs.
//!
//! Sub-display-period numbers -- how much of a frame went to CD reads,
//! to MDEC decode, to VRAM upload -- need a finer clock. Two are
//! available, and only one of them exists on any given target:
//!
//! * [`WideCycles`] is a 32-bit counter at 33.8688 MHz built from root
//!   counter 0 plus its overflow register (RCNT at `0xBF80_1002` is
//!   16-bit and wraps every 1.94 ms, too short to measure a single CD
//!   sector transfer). Both halves must be modelled by the target.
//! * [`VblankOnly`] is the fallback: it reports whole display periods
//!   only. Resolution then comes from averaging -- summing a stage over
//!   150 frames of the video gives 1/150 of a display period, about
//!   111 us, which is finer than any stage that matters.
//!
//! [`Clock::detect`] picks between them and records which one it chose,
//! so a report can never silently present 16.67 ms data as if it were
//! finer.

use psx_io::timers::{self, Timer};

/// RCNT: 16-bit free-running counter clocked at 33.8688 MHz. Reported
/// for diagnostics; too short a range to measure with.
const RCNT: *mut u32 = 0xBF80_1002 as *mut u32;
/// Overflow counter for root counter 0.
const OVF0: *mut u32 = 0xBF80_1006 as *mut u32;

/// Microseconds in one NTSC display period.
pub const US_PER_VBLANK: u32 = 1_000_000 / 60;

/// Display periods per video frame when nothing is late: a 60 Hz display
/// showing a 15 fps video.
/// Display periods per video frame. Taken from the shipped player rather
/// than repeated here: a stale copy of this constant made the bench score
/// 6-display-period presents as stutters against a 4-period target.
pub use crate::video::VBLANKS_PER_VIDEO_FRAME;

/// A 32-bit cycle counter at 33.8688 MHz, from root counter 0 extended
/// by its overflow register.
///
/// The (overflow, counter) pair is read twice and compared: a wrap
/// between the two reads would otherwise splice a 2^16 step onto a small
/// delta. Range is 2^32 cycles, about 127 s.
pub struct WideCycles;

impl WideCycles {
    /// Program root counter 0 as a free-running 33.8688 MHz tick source
    /// and zero both halves of the extended counter.
    pub fn init() {
        timers::set_mode(Timer::Timer0, 0x0000);
        timers::set_counter(Timer::Timer0, 0);
        // Clear the overflow last: a wrap that raced the setup above is
        // discarded rather than counted as 2^16 of phantom time.
        write32(OVF0, 0);
        core::hint::spin_loop();
    }

    /// Current value of the extended counter.
    #[inline]
    pub fn now() -> u32 {
        loop {
            let ovf_a = read32(OVF0) & 0xFFFF;
            let val = timers::counter(Timer::Timer0);
            let ovf_b = read32(OVF0) & 0xFFFF;
            if ovf_a == ovf_b {
                return (ovf_a << 16) | val as u32;
            }
        }
    }

    /// True when the target counts wraps as well as the low 16 bits.
    ///
    /// Probed, never assumed: DuckStation models root counter 0 but not
    /// its overflow register, and RCNT is unimplemented there, so a
    /// naive `(ovf << 16) | val` would alias and report nonsense as if
    /// it were cycle-accurate. Requiring the overflow register to move
    /// across a wait that is known to span many wraps is the only way to
    /// tell a real counter from a dead one.
    pub fn overflow_is_live() -> bool {
        let before = read32(OVF0);
        let v0 = Vblanks::now();
        while Vblanks::now() == v0 + 3 {
            core::hint::spin_loop();
        }
        read32(OVF0) != before
    }
}

/// Display-period counter, mirrored from the VBlank IRQ handler that
/// `psx_rt` installs.
pub struct Vblanks;

impl Vblanks {
    /// Display periods since the VBlank counter was installed.
    #[inline(always)]
    pub fn now() -> u32 {
        psx_rt::interrupts::vblank_count()
    }

    /// Block until the next display period.
    #[inline]
    pub fn wait() {
        psx_rt::interrupts::wait_vblank();
    }
}

/// Which clock a [`Clock`] reads.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ClockKind {
    /// Whole display periods only. Sub-period resolution comes from
    /// averaging over many samples.
    Vblank,
    /// 33.8688 MHz cycles, 32 bits wide.
    Cycles,
}

impl ClockKind {
    pub const fn name(self) -> &'static str {
        match self {
            ClockKind::Vblank => "vblank",
            ClockKind::Cycles => "cycles",
        }
    }

    /// Stable numeric tag for the TTY report.
    pub const fn code(self) -> u32 {
        match self {
            ClockKind::Vblank => 0,
            ClockKind::Cycles => 1,
        }
    }
}

/// One span of elapsed time, as whatever the detected clock can measure.
#[derive(Copy, Clone, Default)]
pub struct Span {
    pub vblanks: u32,
    pub cycles: u32,
}

impl core::ops::Add for Span {
    type Output = Span;
    fn add(self, other: Span) -> Span {
        Span {
            vblanks: self.vblanks.wrapping_add(other.vblanks),
            cycles: self.cycles.wrapping_add(other.cycles),
        }
    }
}

impl core::ops::AddAssign for Span {
    fn add_assign(&mut self, other: Span) {
        self.vblanks = self.vblanks.wrapping_add(other.vblanks);
        self.cycles = self.cycles.wrapping_add(other.cycles);
    }
}

/// A running stopwatch over whichever clock was detected.
pub struct Clock {
    kind: ClockKind,
    vblanks: u32,
    cycles: u32,
    last: Span,
}

impl Clock {
    /// Probe the target and return a clock that works on it.
    pub fn detect() -> Self {
        WideCycles::init();
        let kind = if WideCycles::overflow_is_live() {
            ClockKind::Cycles
        } else {
            ClockKind::Vblank
        };
        Self::of(kind)
    }

    /// A clock of a known kind, for interpreting spans captured
    /// elsewhere without re-probing the target.
    pub fn of(kind: ClockKind) -> Self {
        Self {
            kind,
            vblanks: 0,
            cycles: 0,
            last: Span::default(),
        }
    }

    pub const fn kind(&self) -> ClockKind {
        self.kind
    }

    /// Re-arm the stopwatch.
    #[inline]
    pub fn start(&mut self) {
        self.vblanks = Vblanks::now();
        self.cycles = WideCycles::now();
        self.last = Span::default();
    }

    /// Elapsed time since the last [`Clock::start`] or [`Clock::lap`],
    /// without disturbing it.
    #[inline]
    pub fn peek(&self) -> Span {
        Span {
            vblanks: Vblanks::now().wrapping_sub(self.vblanks),
            cycles: WideCycles::now().wrapping_sub(self.cycles),
        }
    }

    /// Elapsed time since the last [`Clock::start`] or [`Clock::lap`],
    /// and re-arm.
    #[inline]
    pub fn lap(&mut self) -> Span {
        let span = self.peek();
        self.vblanks = self.vblanks.wrapping_add(span.vblanks);
        self.cycles = self.cycles.wrapping_add(span.cycles);
        self.last = span;
        span
    }

    /// The most recent [`Clock::lap`] result.
    #[inline]
    pub fn last(&self) -> Span {
        self.last
    }

    /// Value of the finer field for this clock: cycles when the target
    /// has a cycle counter, whole display periods otherwise. Aggregated
    /// over many samples this is the headline cost figure; with a cycle
    /// counter it is exact per sample.
    #[inline]
    pub fn primary(&self, span: Span) -> u32 {
        match self.kind {
            ClockKind::Cycles => span.cycles,
            ClockKind::Vblank => span.vblanks,
        }
    }

    /// Microseconds per sample implied by `span` accumulated over
    /// `samples` iterations of that stage.
    ///
    /// With a cycle counter this is exact. Without one, `span.vblanks` is
    /// a whole number of display periods, so dividing the total by the
    /// sample count is what recovers sub-period resolution: 150 frames of
    /// display-period totals resolve to about 111 us.
    pub fn us_per_sample(&self, span: Span, samples: u32) -> u32 {
        let samples = core::cmp::max(samples, 1) as u64;
        match self.kind {
            ClockKind::Cycles => (span.cycles as u64 * 1_000_000 / 33_868_800 / samples) as u32,
            ClockKind::Vblank => (span.vblanks as u64 * US_PER_VBLANK as u64 / samples) as u32,
        }
    }
}

/// What the cycle counters actually did, recorded so a report can be
/// read honestly.
pub struct CounterProbe {
    /// Clock selected for fine measurement.
    pub clock: ClockKind,
    /// RCNT delta across one display period. Zero means unimplemented.
    pub rcnt_per_vblank: u32,
    /// Root counter 0 delta across one display period, 16-bit and
    /// therefore aliased, reported only to show the register is live.
    pub t0_per_vblank_aliased: u32,
    /// Root counter 0 delta across a fixed-cost empty loop, aliased.
    pub t0_per_spin_aliased: u32,
    /// Overflow-register movement across three display periods.
    pub overflow_moved: bool,
    /// Cycles the extended counter reports for one display period. Only
    /// meaningful when [`CounterProbe::overflow_moved`] is true.
    pub cycles_per_vblank: u32,
}

/// Measure the cycle counters against a known reference.
pub fn probe_counters() -> CounterProbe {
    // One display period, as measured by the VBlank IRQ: the only
    // reference the guest can trust absolutely.
    let v0 = Vblanks::now();
    let rcnt0 = read32(RCNT);
    let t0 = WideCycles::now();
    let ovf0 = read32(OVF0);
    while Vblanks::now() == v0 {
        core::hint::spin_loop();
    }
    let rcnt_per_vblank = read32(RCNT).wrapping_sub(rcnt0) & 0xFFFF;
    let t0_per_vblank_aliased = (timers::counter(Timer::Timer0)).wrapping_sub(t0 as u16) as u32;
    let cycles_per_vblank = WideCycles::now().wrapping_sub(t0);
    let overflow_moved = read32(OVF0) != ovf0;

    // Fixed-cost empty loop: a plausible cycles-per-iteration figure
    // (a handful) means the counter tracks the CPU.
    let c1 = WideCycles::now();
    let mut acc = 0u32;
    for i in 0..1_000_000u32 {
        acc = acc.wrapping_add(i);
    }
    let t0_per_spin_aliased = (timers::counter(Timer::Timer0)).wrapping_sub(c1 as u16) as u32;
    core::hint::black_box(acc);

    let clock = if overflow_moved {
        ClockKind::Cycles
    } else {
        ClockKind::Vblank
    };

    CounterProbe {
        clock,
        rcnt_per_vblank,
        t0_per_vblank_aliased,
        t0_per_spin_aliased,
        overflow_moved,
        cycles_per_vblank,
    }
}

/// Running summary of one measured quantity.
///
/// Percentiles are exact over the retained samples, which matters
/// because a single 150-frame video produces few of them. A histogram is
/// kept alongside for the on-screen view, in units of 1/16 of a display
/// period once the totals have been divided by a sample count.
pub struct Stats {
    pub count: u32,
    pub sum: u64,
    pub min: u32,
    pub max: u32,
    pub hist: [u32; HIST_BUCKETS],
    pub samples: [u32; MAX_SAMPLES],
    pub sample_count: u32,
}

pub const HIST_BUCKETS: usize = 32;
/// Histogram bucket width, in units of the summed total: a bucket is
/// `HIST_SCALE`/HIST_BUCKETS of the total, so the same shape works for
/// a 1-sample and a 150-sample set.
pub const HIST_SCALE: u32 = 1 << 20;
/// Values retained for exact percentiles. A 150-frame video plus a
/// stage-attribution pass fits without truncation.
pub const MAX_SAMPLES: usize = 192;

impl Stats {
    pub const fn new() -> Self {
        Self {
            count: 0,
            sum: 0,
            min: u32::MAX,
            max: 0,
            hist: [0; HIST_BUCKETS],
            samples: [0; MAX_SAMPLES],
            sample_count: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, value: u32) {
        self.count += 1;
        self.sum += value as u64;
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
        if (self.sample_count as usize) < MAX_SAMPLES {
            self.samples[self.sample_count as usize] = value;
            self.sample_count += 1;
        }
    }

    /// Push `value` into the histogram scaled by `scale`, for sets whose
    /// samples are integers but whose totals are worth plotting.
    pub fn push_scaled(&mut self, value: u32, scale: u32) {
        let scaled = (value as u64 * HIST_SCALE as u64 / core::cmp::max(scale, 1) as u64) as u32;
        let bucket = core::cmp::min((scaled >> 16) as usize, HIST_BUCKETS - 1);
        self.hist[bucket] += 1;
    }

    /// Mean, or 0 for an empty set.
    pub fn mean(&self) -> u32 {
        if self.count == 0 {
            0
        } else {
            (self.sum / self.count as u64) as u32
        }
    }

    /// Exact percentile over the retained samples (nearest-rank), in the
    /// sample's own units.
    pub fn percentile(&self, percent: u32) -> u32 {
        if self.sample_count == 0 {
            return 0;
        }
        let mut sorted = [0u32; MAX_SAMPLES];
        sorted[..self.sample_count as usize]
            .copy_from_slice(&self.samples[..self.sample_count as usize]);
        sorted[..self.sample_count as usize].sort_unstable();
        let rank = ((percent * self.sample_count) / 100).max(1) as usize;
        sorted[core::cmp::min(rank, self.sample_count as usize) - 1]
    }

    /// Population standard deviation over the retained samples, scaled by
    /// 100. Integer Newton sqrt: `no_std` has no float sqrt.
    pub fn stddev_x100(&self) -> u32 {
        if self.sample_count < 2 {
            return 0;
        }
        let n = self.sample_count as u64;
        let mean = self.sum / self.count as u64;
        let mut acc = 0u64;
        for i in 0..self.sample_count as usize {
            let d = self.samples[i] as i64 - mean as i64;
            acc += (d * d) as u64;
        }
        // Variance scaled by 10_000, so the sqrt below yields cycles*100.
        let var_x10000 = (acc / n) * 10_000;
        let mut root = isqrt(var_x10000);
        for _ in 0..4 {
            root = (root + core::cmp::max(var_x10000 / root.max(1), 1)) / 2;
        }
        root as u32
    }

    /// Total, in the sample's own units.
    pub fn total(&self) -> u64 {
        self.sum
    }
}

/// Integer square root.
fn isqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[inline(always)]
fn read32(addr: *mut u32) -> u32 {
    unsafe { core::ptr::read_volatile(addr) }
}

#[inline(always)]
fn write32(addr: *mut u32, value: u32) {
    unsafe { core::ptr::write_volatile(addr, value) }
}
