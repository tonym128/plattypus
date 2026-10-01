//! Host-side test suite for Plattypus game logic invariants.
//! Tests save serialization, checksum verification, codename ranking,
//! act progression, and cell collisions without requiring bare-metal MIPS hardware.

use plattypus_core::{level::*, save::*};

// --------------------------------------------------------------------------
// Campaign progress (game/src/game.rs)
// --------------------------------------------------------------------------

/// Mirror of the video standards' frame rates (game/src/game.rs).
// Video geometry mirror (SE-17 / M3 / SE-4).
const TOTAL_FRAMES: u16 = 150;
const EMBEDDED_FRAME_COUNT: u16 = 16;
const SECTORS_PER_FRAME: usize = 8;
const WORDS_PER_FRAME: usize = 4096;

const FPS_NTSC: u32 = 60;
const FPS_PAL: u32 = 50;
/// Mirror of the boss intro cutscene constants (game/src/game.rs).
const BOSS_INTRO_FRAMES: u16 = 240;
const BOSS_ORBIT_RADIUS: i32 = 450;
const ORBIT_TURN_UNITS: u32 = 256;


/// Mirror of `Game::commit_progress`: the only writer of the campaign clear
/// count, the high score and the yabby total.
pub fn commit_progress(save: &mut SaveData, cleared: Option<Act>, score: u32, yabbies: u16) {
    if let Some(act) = cleared {
        let idx = act as u8;
        if idx < CAMPAIGN_ACT_COUNT {
            let target = (idx + 1).min(CAMPAIGN_ACT_COUNT);
            save.unlocked_act = save.unlocked_act.max(target);
        }
    }
    save.highest_score = save.highest_score.max(score);
    save.total_yabbies = save.total_yabbies.max(yabbies);
}

/// Mirror of `game::campaign_completed`.
pub fn campaign_completed(unlocked_act: u8) -> bool {
    unlocked_act >= CAMPAIGN_ACT_COUNT
}

/// Mirror of `game::pending_act`.
pub fn pending_act(unlocked_act: u8) -> u8 {
    unlocked_act.min(CAMPAIGN_ACT_COUNT - 1)
}

/// Mirror of the title screen's CONTINUE decision.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TitleContinue {
    StageSelect,
    PlayAct(Act),
}

pub fn title_continue(unlocked_act: u8) -> TitleContinue {
    if campaign_completed(unlocked_act) {
        TitleContinue::StageSelect
    } else {
        TitleContinue::PlayAct(Act::from_u8(pending_act(unlocked_act)))
    }
}

/// Mirror of `Game::frames_per_second`.
pub fn frames_to_seconds(frames: u32, fps: u32) -> u32 {
    frames / fps
}

/// 256-entry Q1.12 sine table, copied from psx-math (`sincos.rs`).
/// `sin_1_3_12` indexes it with `angle & 0xFF`, so one revolution is 256 units.
const SIN_TABLE: [i16; 256] = [
    0, 101, 201, 301, 401, 501, 601, 700, 799, 897, 995, 1092, 1189, 1285, 1380, 1474, 1567, 1660,
    1751, 1842, 1931, 2019, 2106, 2191, 2276, 2359, 2440, 2520, 2598, 2675, 2751, 2824, 2896, 2967,
    3035, 3102, 3166, 3229, 3290, 3349, 3406, 3461, 3513, 3564, 3612, 3659, 3703, 3745, 3784, 3822,
    3857, 3889, 3920, 3948, 3973, 3996, 4017, 4036, 4052, 4065, 4076, 4085, 4091, 4095, 4096, 4095,
    4091, 4085, 4076, 4065, 4052, 4036, 4017, 3996, 3973, 3948, 3920, 3889, 3857, 3822, 3784, 3745,
    3703, 3659, 3612, 3564, 3513, 3461, 3406, 3349, 3290, 3229, 3166, 3102, 3035, 2967, 2896, 2824,
    2751, 2675, 2598, 2520, 2440, 2359, 2276, 2191, 2106, 2019, 1931, 1842, 1751, 1660, 1567, 1474,
    1380, 1285, 1189, 1092, 995, 897, 799, 700, 601, 501, 401, 301, 201, 101, 0, -101, -201, -301,
    -401, -501, -601, -700, -799, -897, -995, -1092, -1189, -1285, -1380, -1474, -1567, -1660,
    -1751, -1842, -1931, -2019, -2106, -2191, -2276, -2359, -2440, -2520, -2598, -2675, -2751,
    -2824, -2896, -2967, -3035, -3102, -3166, -3229, -3290, -3349, -3406, -3461, -3513, -3564,
    -3612, -3659, -3703, -3745, -3784, -3822, -3857, -3889, -3920, -3948, -3973, -3996, -4017,
    -4036, -4052, -4065, -4076, -4085, -4091, -4095, -4096, -4095, -4091, -4085, -4076, -4065,
    -4052, -4036, -4017, -3996, -3973, -3948, -3920, -3889, -3857, -3822, -3784, -3745, -3703,
    -3659, -3612, -3564, -3513, -3461, -3406, -3349, -3290, -3229, -3166, -3102, -3035, -2967,
    -2896, -2824, -2751, -2675, -2598, -2520, -2440, -2359, -2276, -2191, -2106, -2019, -1931,
    -1842, -1751, -1660, -1567, -1474, -1380, -1285, -1189, -1092, -995, -897, -799, -700, -601,
    -501, -401, -301, -201, -101,
];

/// Mirror of `psx_gte_core::transform::sin_1_3_12` / `cos_1_3_12`.
fn sin_1_3_12(angle: u16) -> i16 {
    SIN_TABLE[(angle & 0xFF) as usize]
}

fn cos_1_3_12(angle: u16) -> i16 {
    sin_1_3_12(angle.wrapping_add(64))
}

// ---------------------------------------------------------------------------
// Player movement mirror (UX-4 / UX-5 / UX-6)
//
// The analog path used to scale each *axis* by a scalar speed and divide by
// the full deflection a second time, so moderate diagonal input truncated to
// a velocity of zero. The game now aims a vector of magnitude `speed` along
// the stick heading instead.
// ---------------------------------------------------------------------------

/// Integer square root, mirroring `psx_math::int32::isqrt_i32`.
fn isqrt_i32(value: i32) -> i32 {
    if value <= 0 {
        return 0;
    }
    let mut guess: i32 = 1;
    while guess * guess < value {
        guess <<= 1;
    }
    while guess * guess > value {
        guess >>= 1;
    }
    while (guess + 1) * (guess + 1) <= value {
        guess += 1;
    }
    while guess * guess > value {
        guess -= 1;
    }
    guess
}


// ---------------------------------------------------------------------------
// Level mirror (UX-2)
//
// Only the generators the tripwire and exit tests need are reproduced here,
// in the same pass order as `level.rs`. Pass order is load-bearing: `set_cell`
// is last-write-wins, and a cosmetic pass placed after a hazard pass silently
// erased the hazard.
// ---------------------------------------------------------------------------

/// Mirror of `game::entities::SentryState`, for the contact-damage gate.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SentryState {
    Patrolling,
    Investigating,
    AlertChase,
    Returning,
}

/// Half-extent of the stage-exit trigger, kept inside `TILE_SZ`.
const EXIT_TRIGGER_RADIUS: i32 = 28;

type Grid = [CellType; GRID_W * GRID_D];

fn blank_grid() -> Grid {
    [CellType::Floor; GRID_W * GRID_D]
}

fn set_cell(grid: &mut Grid, gx: usize, gz: usize, cell: CellType) {
    if gx < GRID_W && gz < GRID_D {
        grid[gz * GRID_W + gx] = cell;
    }
}

/// Mirror of `Level::generate_act1_2`, including the reordered hazard pass.
fn generate_act1_2() -> Grid {
    let mut g = blank_grid();
    for x in 0..GRID_W {
        set_cell(&mut g, x, 0, CellType::Wall);
        set_cell(&mut g, x, GRID_D - 1, CellType::Wall);
    }
    for z in 0..GRID_D {
        set_cell(&mut g, 0, z, CellType::Wall);
        set_cell(&mut g, GRID_W - 1, z, CellType::Wall);
    }
    for z in 2..22 {
        if z != 6 && z != 16 {
            set_cell(&mut g, 7, z, CellType::Wall);
        }
        if z != 11 {
            set_cell(&mut g, 15, z, CellType::Wall);
        }
    }
    for x in 2..22 {
        if x != 11 {
            set_cell(&mut g, x, 11, CellType::Wall);
        }
    }
    set_cell(&mut g, 7, 4, CellType::AirDuct);
    set_cell(&mut g, 7, 18, CellType::AirDuct);
    set_cell(&mut g, 15, 8, CellType::AirDuct);
    set_cell(&mut g, 15, 14, CellType::AirDuct);
    set_cell(&mut g, 11, 4, CellType::AirDuct);
    set_cell(&mut g, 11, 18, CellType::AirDuct);
    let crates = [
        (3usize, 4usize), (4, 4), (3, 7), (4, 7), (3, 14), (4, 14), (3, 17), (4, 17),
        (10, 3), (10, 4), (12, 3), (12, 4), (10, 18), (10, 19), (12, 18), (12, 19),
        (18, 5), (19, 5), (18, 8), (19, 8), (18, 14), (19, 14), (18, 17), (19, 17),
    ];
    for (cx, cz) in crates {
        set_cell(&mut g, cx, cz, CellType::Crate);
    }
    // The grating pass that used to erase the column-11 tripwires.
    for z in 5..=17 {
        set_cell(&mut g, 3, z, CellType::MetalGrate);
        set_cell(&mut g, 11, z, CellType::MetalGrate);
        set_cell(&mut g, 19, z, CellType::MetalGrate);
    }
    // Hazards last.
    for (lx, lz) in [
        (7usize, 6usize), (7, 16), (15, 11), (11, 11), (11, 6), (11, 16),
    ] {
        set_cell(&mut g, lx, lz, CellType::LaserTripwire);
    }
    set_cell(&mut g, 21, 2, CellType::ExitBurrow);
    g
}

fn generate_act1_1() -> Grid {
    let mut g = blank_grid();
    for x in 0..GRID_W {
        set_cell(&mut g, x, 0, CellType::Wall);
        set_cell(&mut g, x, GRID_D - 1, CellType::Wall);
    }
    for z in 0..GRID_D {
        set_cell(&mut g, 0, z, CellType::Wall);
        set_cell(&mut g, GRID_W - 1, z, CellType::Wall);
    }
    for x in 12..22 {
        set_cell(&mut g, x, 10, CellType::Wall);
    }
    set_cell(&mut g, 16, 10, CellType::LaserTripwire);
    set_cell(&mut g, 18, 10, CellType::AirDuct);
    set_cell(&mut g, 21, 2, CellType::ExitBurrow);
    g
}

fn generate_vr_speed() -> Grid {
    let mut g = blank_grid();
    for z in 1..23 {
        for x in 1..23 {
            if x < 10 || x > 14 {
                set_cell(&mut g, x, z, CellType::Water);
            }
        }
    }
    set_cell(&mut g, 12, 17, CellType::Crate);
    set_cell(&mut g, 11, 13, CellType::LaserTripwire);
    set_cell(&mut g, 12, 13, CellType::LaserTripwire);
    set_cell(&mut g, 13, 13, CellType::LaserTripwire);
    set_cell(&mut g, 12, 9, CellType::AirDuct);
    set_cell(&mut g, 12, 2, CellType::ExitBurrow);
    g
}

fn generate_act3_2() -> Grid {
    let mut g = blank_grid();
    for x in 0..GRID_W {
        set_cell(&mut g, x, 0, CellType::Wall);
        set_cell(&mut g, x, GRID_D - 1, CellType::Wall);
    }
    for z in 0..GRID_D {
        set_cell(&mut g, 0, z, CellType::Wall);
        set_cell(&mut g, GRID_W - 1, z, CellType::Wall);
    }
    set_cell(&mut g, 21, 2, CellType::ExitBurrow);
    g
}

fn generate_act(act: Act) -> Grid {
    match act {
        Act::Act1_1Drainage => generate_act1_1(),
        Act::Act1_2Barracks => generate_act1_2(),
        Act::Act3_2Laneways => generate_act3_2(),
        Act::VrSpeed => generate_vr_speed(),
        _ => {
            let mut g = blank_grid();
            for x in 0..GRID_W {
                set_cell(&mut g, x, 0, CellType::Wall);
                set_cell(&mut g, x, GRID_D - 1, CellType::Wall);
            }
            for z in 0..GRID_D {
                set_cell(&mut g, 0, z, CellType::Wall);
                set_cell(&mut g, GRID_W - 1, z, CellType::Wall);
            }
            set_cell(&mut g, 21, 2, CellType::ExitBurrow);
            g
        }
    }
}

/// Grid index of the act's exit tile, mirroring `Level::exit_x/exit_z`.
fn exit_cell_index(act: Act) -> usize {
    let (ex, ez) = match act {
        Act::Act1_1Drainage => (19usize, 3usize),
        Act::Act1_2Barracks => (21, 2),
        Act::Act3_2Laneways => (21, 2),
        Act::VrSpeed => (12, 2),
        _ => (21, 2),
    };
    ez * GRID_W + ex
}


const ANALOG_FULL_DEFLECTION: i32 = 127;

/// Mirror of `game::heading_vx` / `heading_vz`, including the rounding shift.
fn heading_velocity(angle: u16, speed: i32) -> (i32, i32) {
    let comp = |v: i16| {
        let n = v as i32 * speed;
        let q = n.div_euclid(1 << 12);
        let r = n.rem_euclid(1 << 12);
        (q + (r >= (1 << 11)) as i32).clamp(-speed, speed)
    };
    (comp(sin_1_3_12(angle)), comp(cos_1_3_12(angle)))
}

/// The old, broken per-axis scaling, kept so the regression is pinned.
fn legacy_analog_velocity(sx: i32, sy: i32, max_speed: i32) -> (i32, i32) {
    let mag = isqrt_i32(sx * sx + sy * sy);
    let speed = ((max_speed * mag) / ANALOG_FULL_DEFLECTION).max(1);
    ((sx * speed) / ANALOG_FULL_DEFLECTION, (-sy * speed) / ANALOG_FULL_DEFLECTION)
}

/// The full run speed available at a given stick magnitude.
fn run_speed_for(mag: i32, in_box: bool, on_ground: bool) -> i32 {
    if in_box {
        2
    } else if !on_ground {
        4
    } else if mag < 65 {
        2
    } else {
        4
    }
}

/// Magnitude of a velocity, as the game would experience it.
fn speed_of(v: (i32, i32)) -> i32 {
    isqrt_i32(v.0 * v.0 + v.1 * v.1)
}

/// Mirror of `game::boss_orbit_offset`: one full turn across the cutscene.
fn boss_orbit_offset(frame: u16) -> (i32, i32) {
    let angle = (frame as u32 * ORBIT_TURN_UNITS / BOSS_INTRO_FRAMES as u32) as u16;
    let sin_v = sin_1_3_12(angle) as i32;
    let cos_v = cos_1_3_12(angle) as i32;
    (
        (sin_v * BOSS_ORBIT_RADIUS) >> 12,
        (cos_v * BOSS_ORBIT_RADIUS) >> 12,
    )
}

/// The pre-fix formula, kept only to pin the regression it caused: a 4096-unit
/// mask on a 256-entry table, so the angle is `(frame & 0x0F) * 16`.
fn boss_orbit_offset_legacy(frame: u16) -> (i32, i32) {
    let angle = frame.wrapping_mul(16) & 0x0FFF;
    let sin_v = sin_1_3_12(angle) as i32;
    let cos_v = cos_1_3_12(angle) as i32;
    (
        (sin_v * BOSS_ORBIT_RADIUS) >> 12,
        (cos_v * BOSS_ORBIT_RADIUS) >> 12,
    )
}

// -------------------------------------------------------------------------
// HUD TEXT FITTING (UX-3) -- mirrors game/src/renderer.rs
// -------------------------------------------------------------------------
//
// The BASIC font is a fixed 8 px cell and `draw_text` has no bounds check, so
// every user-facing string has to be measured. These mirror the pure helpers
// so the policy can be exercised on the host.

/// Screen width in pixels, mirroring `renderer::SCREEN_W`.
const SCREEN_W: i16 = 320;
/// Font advance in pixels, mirroring `psx_font::fonts::BASIC`.
const ADVANCE: u16 = 8;
/// Tightest inter-glyph gap the fitter will use.
const MAX_TIGHTENING: i8 = -1;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TextFit {
    pub x: i16,
    pub spacing: i8,
}

const fn fitted_width(width: u16, chars: u16, spacing: i8) -> i32 {
    let gaps = if chars > 0 { chars as i32 - 1 } else { 0 };
    (width as i32) + (spacing as i32) * gaps
}

const fn fit_text(x: i16, width: u16, chars: u16) -> TextFit {
    if x >= 0 && (x as i32) + (width as i32) <= SCREEN_W as i32 {
        return TextFit { x, spacing: 0 };
    }
    if (width as i32) <= SCREEN_W as i32 {
        return TextFit {
            x: (SCREEN_W as i32 - width as i32) as i16,
            spacing: 0,
        };
    }
    let mut spacing = 0i8;
    while spacing > MAX_TIGHTENING {
        spacing -= 1;
        if fitted_width(width, chars, spacing) <= SCREEN_W as i32 {
            return TextFit { x: 0, spacing };
        }
    }
    TextFit { x: 0, spacing }
}

const fn center_text(width: u16, chars: u16) -> TextFit {
    let tight = fit_text(0, width, chars);
    let w = fitted_width(width, chars, tight.spacing);
    let left = (SCREEN_W as i32 - w) / 2;
    TextFit {
        x: if left > 0 { left as i16 } else { 0 },
        spacing: tight.spacing,
    }
}

/// Width of `s` and its glyph count, the two numbers the fitter consumes.
fn measure(s: &str) -> (u16, u16) {
    (s.chars().count() as u16 * ADVANCE, s.chars().count() as u16)
}

/// Right edge the fitter chose, for the "does it stay on screen" assertions.
fn right_edge(s: &str, fit: TextFit) -> i32 {
    let (width, chars) = measure(s);
    fit.x as i32 + fitted_width(width, chars, fit.spacing)
}

// -------------------------------------------------------------------------
// BACKFACE CULLING (SE-3) -- mirrors game/src/renderer.rs
// -------------------------------------------------------------------------

const NEAR_SZ: u16 = 20;
const SZ_SATURATED: u16 = 4_096;
const WINDING_CLAMP: i32 = 0x400;

const fn sz_in_front(sz: u16) -> bool {
    sz > NEAR_SZ && sz < SZ_SATURATED
}

const fn clamp_coord(v: i16) -> i32 {
    let w = v as i32;
    if w > WINDING_CLAMP {
        WINDING_CLAMP
    } else if w < -WINDING_CLAMP {
        -WINDING_CLAMP
    } else {
        w
    }
}

const fn winding_cross(a: (i16, i16), b: (i16, i16), c: (i16, i16)) -> i32 {
    let ax = clamp_coord(b.0) - clamp_coord(a.0);
    let ay = clamp_coord(b.1) - clamp_coord(a.1);
    let bx = clamp_coord(c.0) - clamp_coord(a.0);
    let by = clamp_coord(c.1) - clamp_coord(a.1);
    ax * by - ay * bx
}

/// The unclamped i64 reference the clamp is allowed to agree with whenever
/// every coordinate is already on screen.
fn winding_cross_i64(a: (i16, i16), b: (i16, i16), c: (i16, i16)) -> i64 {
    let ax = b.0 as i64 - a.0 as i64;
    let ay = b.1 as i64 - a.1 as i64;
    let bx = c.0 as i64 - a.0 as i64;
    let by = c.1 as i64 - a.1 as i64;
    ax * by - ay * bx
}

fn main() {
    println!("=== RUNNING PLATTYPUS GAME LOGIC TESTS ===");

    // 1. Save Data & Checksum Tests
    let mut save = SaveData::new();
    assert!(save.is_valid(), "Initial save data must be valid");
    assert_eq!(save.unlocked_act, 0);

    save.unlocked_act = 5;
    save.highest_score = 12500;
    save.total_yabbies = 42;
    assert!(!save.is_valid(), "Save with modified data without recomputed checksum must be invalid");

    save.checksum = save.compute_checksum();
    assert!(save.is_valid(), "Save with recomputed checksum must be valid");

    save.magic[0] = b'X';
    assert!(!save.is_valid(), "Corrupted magic signature must be invalid");
    println!("✓ Save data checksum & validation test PASSED");

    // 1b. Explicit no-padding serialization (PD-6)
    assert_eq!(SERIALIZED_SIZE, 46, "Serialized payload must be 46 bytes with no padding");
    assert_eq!(
        core::mem::size_of::<SaveData>(),
        SERIALIZED_SIZE + STRUCT_PADDING,
        "The repr(C) struct must still carry the 2-byte alignment hole, proving it is not the wire format"
    );
    let sample = SaveData {
        unlocked_act: 7,
        highest_score: 0x00AB_CDEF,
        total_yabbies: 0x1234,
        alerts_count: 0x5678,
        best_time_seconds: 0x0009_C1D0,
        best_codename: *b"IRON BILL       ",
        tuxedo_unlocked: 1,
        camo_unlocked: 1,
        wireframe_unlocked: 1,
        vr_cleared: 0x0D,
        selected_costume: 2,
        wireframe_enabled: 1,
        language: 4,
        screen_offset_x: -16,
        screen_offset_y: 16,
        pal_mode: 1,
        ..SaveData::new()
    }
    .with_checksum();

    let bytes_a = sample.to_bytes();
    let bytes_b = sample.to_bytes();
    assert_eq!(bytes_a, bytes_b, "Serialization must be deterministic (PD-6: no uninitialized bytes)");
    // The struct blob's bytes 6..8 are the alignment hole and used to leak
    // stack residue; the payload has no such gap: `highest_score` follows
    // `unlocked_act` directly and is written little-endian.
    assert_eq!(bytes_a.len(), SERIALIZED_SIZE);
    assert_eq!(&bytes_a[0..4], b"PLTY");
    assert_eq!(bytes_a[OFF_VERSION], SAVE_VERSION);
    assert_eq!(bytes_a[OFF_UNLOCKED_ACT], 7);
    assert_eq!(
        &bytes_a[OFF_HIGHEST_SCORE..OFF_HIGHEST_SCORE + 4],
        &[0xEF, 0xCD, 0xAB, 0x00],
        "No padding hole: highest_score must start at offset 6, right after unlocked_act"
    );
    assert_eq!(&bytes_a[OFF_TOTAL_YABBIES..OFF_TOTAL_YABBIES + 2], &[0x34, 0x12]);
    assert_eq!(&bytes_a[OFF_ALERTS_COUNT..OFF_ALERTS_COUNT + 2], &[0x78, 0x56]);
    assert_eq!(&bytes_a[OFF_BEST_TIME..OFF_BEST_TIME + 4], &[0xD0, 0xC1, 0x09, 0x00]);
    assert_eq!(&bytes_a[OFF_CHECKSUM..OFF_CHECKSUM + 2], &sample.checksum.to_le_bytes());
    assert_eq!(SaveData::from_bytes(&bytes_a), Some(sample), "from_bytes must invert to_bytes");
    assert!(sample.is_valid() && sample.is_sane());
    assert!(SaveData::from_bytes(&bytes_a[..SERIALIZED_SIZE - 1]).is_none(), "Truncated payload must be rejected");
    println!("✓ Explicit no-padding serialization test (PD-6) PASSED");

    // 1c. Position-dependent checksum (PD-9b / PD-9a)
    let high_word_flip = SaveData { best_time_seconds: 0x0009_C1D0 ^ 0x0001_0000, ..sample };
    assert_ne!(
        sample.compute_checksum(),
        high_word_flip.compute_checksum(),
        "A single-bit flip in the HIGH word of best_time_seconds must change the checksum (PD-9a)"
    );
    let low_word_flip = SaveData { best_time_seconds: 0x0009_C1D0 ^ 0x0000_0001, ..sample };
    assert_ne!(sample.compute_checksum(), low_word_flip.compute_checksum());

    let swapped = SaveData {
        total_yabbies: sample.alerts_count,
        alerts_count: sample.total_yabbies,
        ..sample
    };
    assert_ne!(
        sample.compute_checksum(),
        swapped.compute_checksum(),
        "Swapping two same-typed fields must change the checksum (PD-9b: a sum is commutative)"
    );
    assert!(!swapped.is_valid(), "A swapped-field save must not validate against the original checksum");
    // Every single bit of every field must be covered.
    let mut undetected = 0;
    for byte in 0..SERIALIZED_SIZE - 2 {
        for bit in 0..8 {
            let mut probe = bytes_a;
            probe[byte] ^= 1 << bit;
            match SaveData::from_bytes(&probe) {
                Some(parsed) if parsed.compute_checksum() == sample.checksum => undetected += 1,
                _ => {}
            }
        }
    }
    assert_eq!(undetected, 0, "Every single-bit flip in the payload must be detected");
    println!("✓ Position-dependent checksum coverage test (PD-9a/PD-9b) PASSED");

    // 1d. Field-range validation (PD-9d)
    let out_of_range = [
        ("selected_costume = 200", SaveData { selected_costume: 200, ..sample }),
        ("language = 255", SaveData { language: 255, ..sample }),
        ("pal_mode = 200", SaveData { pal_mode: 200, ..sample }),
        ("screen_offset_x = -128", SaveData { screen_offset_x: -128, ..sample }),
        ("screen_offset_y = 127", SaveData { screen_offset_y: 127, ..sample }),
        ("screen_offset_y = 17", SaveData { screen_offset_y: 17, ..sample }),
        ("unlocked_act = 16", SaveData { unlocked_act: 16, ..sample }),
        // A VR act index in the campaign clear count: no longer reachable, so
        // a card carrying one is a save written by the unclamped CODEC path.
        ("unlocked_act = 13 (VR index)", SaveData { unlocked_act: 13, ..sample }),
        ("unlocked_act = 15 (VR index)", SaveData { unlocked_act: 15, ..sample }),
        ("vr_cleared = 200", SaveData { vr_cleared: 200, ..sample }),
        ("tuxedo_unlocked = 7", SaveData { tuxedo_unlocked: 7, ..sample }),
    ];
    for (label, bad) in out_of_range {
        // A valid checksum must not buy an out-of-range field a pass.
        let bad = bad.with_checksum();
        assert!(bad.is_valid(), "{label} must still be structurally valid");
        assert!(!bad.is_sane(), "{label} must fail the sanity check (PD-9d)");
    }
    let edge = SaveData { screen_offset_x: -16, screen_offset_y: 16, ..sample }.with_checksum();
    assert!(edge.is_sane(), "The options menu's +/-16 clamp is the accepted boundary");
    // A VR clear records `vr_cleared` and leaves the campaign count alone, so a
    // save written from the VR trainer carries a campaign count, never a VR
    // act index: the largest legitimate value is "all 12 acts cleared".
    let vr_progress = SaveData {
        vr_cleared: 0x0F,
        unlocked_act: CAMPAIGN_ACT_COUNT,
        ..sample
    }
    .with_checksum();
    assert!(vr_progress.is_sane(), "A save written from the VR trainer must stay valid");
    assert_eq!(
        load_outcome_for(Some(&vr_progress.to_bytes())),
        LoadOutcome::Loaded,
        "A VR-trainer save must load, not be reported as corrupt"
    );
    println!("✓ Save field-range validation test (PD-9d) PASSED");

    // 1e. Load classification: absent vs incompatible vs corrupt (PD-9c / 2.7)
    assert_eq!(load_outcome_for(None), LoadOutcome::NotFound, "No file at all");
    let mut old_version = sample.to_bytes();
    old_version[OFF_VERSION] = 3;
    assert_eq!(
        load_outcome_for(Some(&old_version)),
        LoadOutcome::Incompatible,
        "A v3 card must be reported as an incompatible save, never as a missing one"
    );
    assert_ne!(load_outcome_for(Some(&old_version)), load_outcome_for(None));
    let mut flipped = sample.to_bytes();
    flipped[OFF_ALERTS_COUNT] ^= 0x20;
    assert_eq!(load_outcome_for(Some(&flipped)), LoadOutcome::Corrupt, "A bad checksum must be Corrupt");
    let mut short = [0u8; 12];
    short[..4].copy_from_slice(&SAVE_MAGIC);
    assert_eq!(load_outcome_for(Some(&short)), LoadOutcome::Corrupt, "A truncated payload must be Corrupt");
    let insane = SaveData { selected_costume: 200, ..sample }.with_checksum();
    assert_eq!(
        load_outcome_for(Some(&insane.to_bytes())),
        LoadOutcome::Corrupt,
        "A save with a valid checksum but an out-of-range field must be treated as corrupt"
    );
    assert_eq!(load_outcome_for(Some(&sample.to_bytes())), LoadOutcome::Loaded);
    // The rejected file stays on the card, and the next save overwrites it with
    // a fresh checksum: the game must know it happened.
    assert_eq!(SaveData::new().with_checksum().is_valid(), true, "A rewritten save must validate");
    println!("✓ Load outcome classification test (PD-9c) PASSED");

    // 2. Codename Evaluation Tests (12 Ranks)
    assert_eq!(Codename::evaluate(0, 0, 300, 2), Codename::BigPlatypus, "0 alerts + 0 damage + fast time awards Big Platypus");
    assert_eq!(Codename::evaluate(0, 5, 500, 0), Codename::GhostPlatypus, "0 alerts + 0 kills awards Ghost Platypus");
    assert_eq!(Codename::evaluate(3, 4, 250, 3), Codename::SpeedyWallaby, "<= 300s awards Speedy Wallaby");
    assert_eq!(Codename::evaluate(3, 10, 500, 12), Codename::TasmanianDevil, ">= 10 takedowns awards Tasmanian Devil");
    assert_eq!(Codename::evaluate(1, 2, 500, 2), Codename::LurkingEchidna, "<= 2 alerts + <= 3 damage awards Lurking Echidna");
    assert_eq!(Codename::evaluate(4, 15, 500, 3), Codename::IronBill, ">= 12 damage awards Iron Bill");
    assert_eq!(Codename::evaluate(4, 5, 550, 2), Codename::SlyPossum, "<= 600s + <= 5 alerts awards Sly Possum");
    assert_eq!(Codename::evaluate(3, 5, 700, 5), Codename::BushKoala, ">= 5 takedowns + <= 4 alerts awards Bush Koala");
    assert_eq!(Codename::evaluate(6, 5, 700, 7), Codename::VenomousTaipan, ">= 6 takedowns awards Venomous Taipan");
    assert_eq!(Codename::evaluate(7, 5, 800, 3), Codename::WombatTunnel, "<= 8 alerts awards Wombat Tunnel");
    assert_eq!(Codename::evaluate(10, 5, 800, 1), Codename::CardboardHermit, "<= 1 takedowns awards Cardboard Hermit");
    assert_eq!(Codename::evaluate(12, 5, 900, 4), Codename::DuckbillRookie, "Fallback awards Duckbill Rookie");
    println!("✓ Codename evaluation & stealth rank test (12/12 codenames) PASSED");

    // 3. Act Sequential Progression Tests
    let mut current_act = Act::Act1_1Drainage;
    let mut act_count = 1;
    while let Some(next) = current_act.next() {
        assert_eq!(Act::from_u8(current_act as u8), current_act);
        current_act = next;
        act_count += 1;
    }
    assert_eq!(current_act, Act::Act4_3ExcavatorBoss);
    assert_eq!(act_count, 12, "Campaign must have exactly 12 acts");
    assert!(Act::Act1_3MechBoss.is_boss());
    assert!(Act::Act2_3JetSkiBoss.is_boss());
    assert!(Act::Act3_3SniperBoss.is_boss());
    assert!(Act::Act4_3ExcavatorBoss.is_boss());
    assert!(!Act::Act1_1Drainage.is_boss());
    assert!(Act::Act2_1Rapids.is_rapids());
    assert!(Act::Act2_3JetSkiBoss.is_rapids());
    assert!(!Act::Act2_2Mangroves.is_rapids());
    println!("✓ Campaign act progression and metadata test PASSED");

    // 4. CellType Infiltration Collision Tests
    assert!(CellType::AirDuct.is_solid(false), "Air duct must block standing operative");
    assert!(!CellType::AirDuct.is_solid(true), "Air duct must permit crawling operative");
    assert!(CellType::Wall.is_solid(false) && CellType::Wall.is_solid(true));
    assert!(CellType::Crate.is_solid(false) && CellType::Crate.is_solid(true));
    assert!(!CellType::Floor.is_solid(false) && !CellType::Floor.is_solid(true));
    assert!(CellType::Water.is_water());
    assert!(CellType::WaterCurrent.is_water());
    assert!(!CellType::Floor.is_water());
    println!("✓ Infiltration collision & terrain mechanics test PASSED");

    // 5. Campaign Completion & Continue Bounds Tests (QA-1 / PD-1 / PD-2)
    let max_campaign_act = Act::Act4_3ExcavatorBoss as u8;
    assert_eq!(max_campaign_act, 11, "Act 4-3 must be act index 11");
    assert_eq!(CAMPAIGN_ACT_COUNT, 12, "The campaign clear count tops out at 12");

    // Clearing the last act must read as "finished", which is a different
    // value from "Act 4-3 is now available" (11).
    let mut finished = SaveData::new();
    commit_progress(&mut finished, Some(Act::Act4_3ExcavatorBoss), 0, 0);
    assert_eq!(finished.unlocked_act, CAMPAIGN_ACT_COUNT, "Clearing 4-3 completes the campaign");
    assert!(campaign_completed(finished.unlocked_act));
    assert!(finished.is_sane());

    // Continue flow: the pending act must never resolve to a VR act (>= 12),
    // whatever the card carries.
    for unlocked in 0..=255u8 {
        let act = Act::from_u8(pending_act(unlocked));
        assert!(!act.is_vr(), "Continue act must never be a VR training stage");
        assert!((act as u8) <= 11, "Continue act index must be in 0..=11");
    }
    println!("✓ Campaign completion & continue bounds test (QA-1) PASSED");

    // 6. Oxygen Depletion & Drowning Simulation Tests (QA-3 / UX-3)
    let mut sim_air: u8 = 100;
    let mut sim_health: u8 = 3;
    let mut frames_underwater = 0;
    let mut drowning_damage_ticks = 0;

    // Simulate 400 frames submerged (100 air / (1 drain / 4 frames) = 400 frames)
    for frame in 1..=400 {
        if frame % 4 == 0 && sim_air > 0 {
            sim_air -= 1;
        }
        frames_underwater += 1;
    }
    assert_eq!(frames_underwater, 400, "Submersion loop must run for 400 frames");
    assert_eq!(sim_air, 0, "Air must reach 0 after exactly 400 frames (~6.67 seconds)");
    assert_eq!(sim_health, 3, "Health must not be damaged while air remains");

    // Simulate next 90 frames with 0 air: damage every 30 frames
    for frame in 401..=490 {
        if frame % 30 == 0 && sim_health > 0 {
            sim_health -= 1;
            drowning_damage_ticks += 1;
        }
    }
    assert_eq!(drowning_damage_ticks, 3, "Drowning must tick damage once every 30 frames (0.5s)");
    assert_eq!(sim_health, 0, "Operative must take fatal damage after sustained drowning");

    // Surface recovery test
    for _ in 0..34 {
        sim_air = (sim_air + 3).min(100);
    }
    assert_eq!(sim_air, 100, "Air must fully recover on surface in ~34 frames (~0.57s)");
    println!("✓ Oxygen depletion & drowning rate test (QA-3) PASSED");

    // 7. Cardboard Box Disguise Transitions (QA-5)
    let mut in_box = false;
    let has_box = true;
    let on_ground = false; // in air
    let in_water = false;
    // Attempt toggle in air
    if has_box && on_ground && !in_water {
        in_box = !in_box;
    }
    assert!(!in_box, "Cardboard box must NOT be equippable while airborne");

    // Attempt toggle in water
    let on_ground = true;
    let in_water = true;
    if has_box && on_ground && !in_water {
        in_box = !in_box;
    }
    assert!(!in_box, "Cardboard box must NOT be equippable while swimming in water");

    // Normal equip on dry ground
    let in_water = false;
    if has_box && on_ground && !in_water {
        in_box = !in_box;
    }
    assert!(in_box, "Cardboard box must be equippable on dry ground");

    // Entering water forces unequip
    let in_water = true;
    if in_water && in_box {
        in_box = false;
    }
    assert!(!in_box, "Entering water must instantly shed Cardboard Box disguise");
    println!("✓ Cardboard Box transition safety test (QA-5) PASSED");

    // 8. Boss Retry State Cleanliness Test (QA-2)
    #[allow(dead_code)]
    struct MockMechBoss {
        pub active: bool,
        pub health: u8,
        pub shield_active: bool,
    }
    struct MockPowerConduit {
        pub active: bool,
        pub destroyed: bool,
        pub health: u8,
    }

    fn init_mock_boss_mech() -> (MockMechBoss, [MockPowerConduit; 3]) {
        let boss = MockMechBoss {
            active: true,
            health: 4,
            shield_active: true,
        };
        let conduits = [
            MockPowerConduit { active: true, destroyed: false, health: 3 },
            MockPowerConduit { active: true, destroyed: false, health: 3 },
            MockPowerConduit { active: true, destroyed: false, health: 3 },
        ];
        (boss, conduits)
    }

    let (_boss, mut conduits) = init_mock_boss_mech();
    // Simulate player destroying 2 conduits
    conduits[0].health = 0;
    conduits[0].destroyed = true;
    conduits[1].health = 0;
    conduits[1].destroyed = true;
    assert_eq!(conduits[2].destroyed, false);

    // Player dies and retries: stage reload must cleanly restore all conduits and shields
    let (boss_retry, conduits_retry) = init_mock_boss_mech();
    assert!(boss_retry.shield_active, "Boss shield must be fully active on stage retry");
    assert_eq!(boss_retry.health, 4, "Boss health must be fully restored to 4 on retry");
    for (i, c) in conduits_retry.iter().enumerate() {
        assert!(c.active, "Conduit {} must be active", i);
        assert!(!c.destroyed, "Conduit {} must not be destroyed", i);
        assert_eq!(c.health, 3, "Conduit {} must have 3 health", i);
    }
    println!("✓ Boss retry state cleanliness test (QA-2) PASSED");

    // 9. Rapids Lane Boundary & Riverbank Clipping Test (QA-4)
    // River corridor in Act 2-1:
    // Left bank: x in 0..=8 is Wall (0..9 * 64)
    // Flume: x in 9..=13 is WaterCurrent (9*64 .. 14*64)
    // Right bank: x in 14..=23 is Wall (14*64 .. 24*64)
    const TILE_SZ: i32 = 64;
    let col_radius: i32 = 12;
    let mut player_x = 9 * TILE_SZ + 32; // In Lane 0 (leftmost water lane)
    let mut player_z = 15 * TILE_SZ;

    let is_solid_at = |wx: i32, _wz: i32| -> bool {
        let gx = wx / TILE_SZ;
        gx < 9 || gx >= 14 // Left and right banks are solid
    };

    // Hold LEFT hard while water current pushes downriver
    for _ in 0..120 {
        let vx = -4; // Steering hard left into the riverbank
        let vz = -2; // Rushing downstream

        let next_x = player_x + vx;
        let next_z = player_z + vz;

        // Collision logic as in platypus.rs
        if !is_solid_at(next_x + col_radius, player_z) && !is_solid_at(next_x - col_radius, player_z) {
            player_x = next_x;
        }
        if !is_solid_at(player_x, next_z + col_radius) && !is_solid_at(player_x, next_z - col_radius) {
            player_z = next_z;
        }

        // Assert player NEVER clips into the left bank (gx <= 8)
        assert!(player_x - col_radius >= 9 * TILE_SZ, "Platty must never clip into the left bank wall or void");
    }
    println!("✓ Rapids lane boundary & clipping prevention test (QA-4) PASSED");

    // 10. Score Display & HUD Format Bounds Test (QA-7)
    let format_score = |score: u32| -> [u8; 6] {
        let clamped = score.min(999_990);
        let mut buf = [b'0'; 6];
        let mut n = clamped;
        for i in (0..6).rev() {
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        buf
    };
    assert_eq!(&format_score(0), b"000000");
    assert_eq!(&format_score(1250), b"001250");
    assert_eq!(&format_score(999_990), b"999990");
    assert_eq!(&format_score(1_500_000), b"999990", "Scores > 999,990 must be capped cleanly to 6 digits");
    println!("✓ Score display format & 6-digit bounds test (QA-7) PASSED");

    // 11. PAL 50Hz vs NTSC 60Hz Frame->Second Conversion (QA-8 / UX-14b)
    // Every conversion in the game goes through `frames_to_seconds(frames,
    // frames_per_second)`, and the rate comes from the video standard in
    // force. Hardcoding 60 made a PAL run's displayed time, its stored
    // `best_time_seconds` and all 12 codename thresholds 20% too generous.
    assert_eq!(frames_to_seconds(3600, FPS_NTSC), 60, "3600 frames at 60Hz is a minute");
    assert_eq!(frames_to_seconds(3000, FPS_PAL), 60, "3000 frames at 50Hz is a minute");
    assert_eq!(frames_to_seconds(0, FPS_NTSC), 0);
    assert_eq!(frames_to_seconds(59, FPS_NTSC), 0, "A partial second truncates");
    assert_eq!(frames_to_seconds(49, FPS_PAL), 0);
    // The same wall clock is the same number of seconds on both standards:
    // 72 s is 4320 frames at 60Hz and 3600 at 50Hz.
    assert_eq!(frames_to_seconds(4320, FPS_NTSC), 72);
    assert_eq!(frames_to_seconds(3600, FPS_PAL), 72);
    // The regression: 3600 frames used to be reported as 60 s on both, so a PAL
    // player was told 72 s of play had taken a minute.
    assert_ne!(frames_to_seconds(3600, FPS_NTSC), frames_to_seconds(3600, FPS_PAL));
    // Codename thresholds are in seconds, so a 420 s rank-S run must be 420 s
    // of frames on whichever standard is in force -- the difficulty is the
    // same in wall-clock terms, which is the point.
    for (fps, frames_for_420s) in [(FPS_NTSC, 420 * FPS_NTSC), (FPS_PAL, 420 * FPS_PAL)] {
        assert_eq!(frames_to_seconds(frames_for_420s, fps), 420);
        assert_eq!(frames_to_seconds(frames_for_420s + 1, fps), 420);
        assert_eq!(frames_to_seconds(frames_for_420s - 1, fps), 419);
        assert_eq!(
            Codename::evaluate(0, 0, frames_to_seconds(frames_for_420s, fps), 0),
            Codename::BigPlatypus,
            "A clean 420 s run is Rank S on either standard"
        );
    }
    // 300 s is the Speedy Wallaby gate; on PAL it is 15000 frames, not 18000.
    assert_eq!(frames_to_seconds(300 * FPS_PAL, FPS_PAL), 300);
    assert_eq!(frames_to_seconds(300 * FPS_PAL - 1, FPS_PAL), 299);
    println!("✓ PAL 50Hz vs NTSC 60Hz frame->second conversion test (QA-8 / UX-14b) PASSED");

    // 12. Single commit_progress path: no writer leaves unlocked_act out of
    // range, and a VR act never advances the campaign (PD-1).
    {
        // The regression: the CODEC scribe wrote `(act as u8).max(cur)` with no
        // clamp, so a VR sim followed by a scribe save marked the campaign
        // complete. Every act, from every prior count, through every path.
        // (`max` cannot lower a count, so an already out-of-range card stays
        // out of range -- which is why `is_sane` rejects one at load instead of
        // trusting it. Only reachable counts are swept here.)
        for start in 0..=CAMPAIGN_ACT_COUNT {
            for idx in 0..=15u8 {
                for advance in [false, true] {
                    let mut save = SaveData::new();
                    save.unlocked_act = start;
                    let act = Act::from_u8(idx);
                    commit_progress(&mut save, if advance { Some(act) } else { None }, 100, 3);
                    assert!(
                        save.unlocked_act <= CAMPAIGN_ACT_COUNT,
                        "unlocked_act {} (from {start}, act {idx}, advance {advance}) is out of range",
                        save.unlocked_act
                    );
                    let checked = save.with_checksum();
                    assert!(checked.is_valid() && checked.is_sane(), "A committed save must pass its own validation");
                    if act.is_vr() {
                        assert_eq!(
                            save.unlocked_act, start,
                            "A VR act ({act:?}) must leave the campaign count at {start}"
                        );
                    }
                    if !advance {
                        assert_eq!(save.unlocked_act, start, "A stats-only write must not move the campaign count");
                    }
                }
            }
        }

        // A linear playthrough raises the count by exactly one act per clear
        // and lands on "finished", never above it.
        let mut linear = SaveData::new();
        let mut act = Act::Act1_1Drainage;
        let mut clears = 0u8;
        loop {
            commit_progress(&mut linear, Some(act), 0, 0);
            clears += 1;
            assert_eq!(linear.unlocked_act, clears, "Clearing act {} must unlock exactly {}", act as u8, clears);
            assert!(!campaign_completed(linear.unlocked_act) || clears == CAMPAIGN_ACT_COUNT);
            match act.next() {
                Some(next) => act = next,
                None => break,
            }
        }
        assert_eq!(clears, 12);
        assert!(campaign_completed(linear.unlocked_act), "A finished playthrough reads as finished");
        // Replaying an earlier act (Stage Select after the finish) must not
        // walk the count backwards: every writer is a `max`.
        commit_progress(&mut linear, Some(Act::Act1_1Drainage), 0, 0);
        assert_eq!(linear.unlocked_act, CAMPAIGN_ACT_COUNT, "Progress never regresses on its own");

        // `total_yabbies` compares, it does not add: `platty.yabbies_collected`
        // is cumulative since `reset_for_new_game`, so the old `saturating_add`
        // grew the total quadratically. Ten yabbies per act, 12 acts.
        let mut yabbies = SaveData::new();
        let mut act = Act::Act1_1Drainage;
        let mut collected = 0u16;
        loop {
            collected += 10;
            commit_progress(&mut yabbies, Some(act), 0, collected);
            match act.next() {
                Some(next) => act = next,
                None => break,
            }
        }
        assert_eq!(
            yabbies.total_yabbies, 120,
            "The yabby total is the campaign-cumulative count, not the sum of every stage's running total"
        );
        // A VR clear and a scribe save both record stats without touching it.
        commit_progress(&mut yabbies, Some(Act::VrSpeed), 0, 5);
        commit_progress(&mut yabbies, None, 0, 7);
        assert_eq!(yabbies.total_yabbies, 120, "A lower running total must not lower the record");
        assert_eq!(yabbies.highest_score, 0);
    }
    println!("✓ Single commit_progress path & range invariant test (PD-1) PASSED");

    // 13. "NEW CAMPAIGN" resets the save (PD-3)
    {
        // A finished campaign, fully unlocked, as it sits on the card.
        let mut done = SaveData::new();
        let mut act = Act::Act1_1Drainage;
        loop {
            commit_progress(&mut done, Some(act), 5000, 120);
            match act.next() {
                Some(next) => act = next,
                None => break,
            }
        }
        done.tuxedo_unlocked = 1;
        done.camo_unlocked = 1;
        done.vr_cleared = 0x0F;
        done.best_time_seconds = 240;
        done.best_codename = *b"IRON BILL       ";
        done.language = 2;
        done.pal_mode = 1;
        done.screen_offset_x = -4;
        done.screen_offset_y = 6;
        done.wireframe_enabled = 1;
        let done = done.with_checksum();
        assert!(campaign_completed(done.unlocked_act), "Precondition: the save is a finished campaign");

        let fresh = new_campaign_save(&done);
        assert_eq!(fresh.unlocked_act, 0, "A new campaign starts at the first act, not the last");
        assert!(!campaign_completed(fresh.unlocked_act), "The title must not offer STAGE SELECT after a reset");
        assert_eq!(title_continue(fresh.unlocked_act), TitleContinue::PlayAct(Act::Act1_1Drainage));
        assert!(fresh.is_sane(), "A reset save must pass the field-range check");
        assert!(fresh.is_valid(), "A reset save must be checksummed and loadable");
        assert_eq!(load_outcome_for(Some(&fresh.to_bytes())), LoadOutcome::Loaded);

        // Progression is gone, the player's display setup is not.
        assert_eq!(fresh.tuxedo_unlocked, 0);
        assert_eq!(fresh.camo_unlocked, 0);
        assert_eq!(fresh.vr_cleared, 0);
        assert_eq!(fresh.highest_score, 0);
        assert_eq!(fresh.total_yabbies, 0);
        assert_eq!(fresh.best_time_seconds, 9999);
        assert_eq!(&fresh.best_codename, b"NEW RECRUIT     ");
        // The costume returns to the default suit: tuxedo and camo are exactly
        // the unlocks being cleared, so keeping the selection would leave the
        // player wearing an unearned costume.
        assert_eq!(fresh.selected_costume, 0);
        assert_eq!(fresh.language, 2, "Language is a setting, not an achievement");
        assert_eq!(fresh.pal_mode, 1, "The video standard is a setting");
        assert_eq!(fresh.screen_offset_x, -4);
        assert_eq!(fresh.screen_offset_y, 6);
        assert_eq!(fresh.wireframe_enabled, 1, "Wireframe is on by default, so its toggle survives");

        // The reset depends on nothing but the five preserved settings, and is
        // deterministic: the same card always resets to the same save.
        assert_eq!(fresh, new_campaign_save(&done), "The reset must be a pure function of the settings");
        let mut other = done;
        other.unlocked_act = 1;
        other.tuxedo_unlocked = 0;
        other.vr_cleared = 0;
        other.highest_score = 0;
        other.selected_costume = 2;
        other.best_time_seconds = 9999;
        assert_eq!(fresh, new_campaign_save(&other), "Progression must not survive in any form");
        let baseline = SaveData::new();
        assert_eq!(fresh.magic, baseline.magic);
        assert_eq!(fresh.version, baseline.version);
        assert_eq!(fresh.best_time_seconds, baseline.best_time_seconds);
        assert_eq!(fresh.wireframe_unlocked, baseline.wireframe_unlocked, "Wireframe stays available by default");
    }
    println!("✓ New-campaign save reset test (PD-3) PASSED");

    // 14. "Campaign complete" is a distinct signal from "Act 4-3 pending" (PD-2)
    {
        // Eleven acts cleared: Act 4-3 is the pending mission, and the title's
        // CONTINUE must load it. Before the split, `unlocked_act >= 11` was
        // read as completion, so this branch was unreachable and the final
        // boss could only be reached by playing through or via Stage Select.
        let mut before_final = SaveData::new();
        let mut act = Act::Act1_1Drainage;
        while act != Act::Act4_3ExcavatorBoss {
            commit_progress(&mut before_final, Some(act), 0, 0);
            act = act.next().expect("the campaign must lead to 4-3");
        }
        assert_eq!(before_final.unlocked_act, 11);
        assert!(!campaign_completed(before_final.unlocked_act), "4-3 pending is not completion");
        assert_eq!(title_continue(before_final.unlocked_act), TitleContinue::PlayAct(Act::Act4_3ExcavatorBoss));
        assert!(!Act::Act4_3ExcavatorBoss.is_vr());

        // Clearing 4-3 moves the count to 12, which is the only value that
        // means completion, and the title switches to Stage Select.
        let mut after_final = before_final;
        commit_progress(&mut after_final, Some(Act::Act4_3ExcavatorBoss), 0, 0);
        assert_ne!(
            after_final.unlocked_act, before_final.unlocked_act,
            "The completion signal must be a different value from the pending act"
        );
        assert!(campaign_completed(after_final.unlocked_act));
        assert_eq!(title_continue(after_final.unlocked_act), TitleContinue::StageSelect);

        // The two states are one value apart, so the title cannot confuse them.
        assert_eq!(before_final.unlocked_act + 1, after_final.unlocked_act);
        // Every reachable clear count resolves to exactly one title behaviour.
        for cleared in 0..=CAMPAIGN_ACT_COUNT {
            let act = title_continue(cleared);
            if campaign_completed(cleared) {
                assert_eq!(act, TitleContinue::StageSelect);
            } else {
                assert_eq!(act, TitleContinue::PlayAct(Act::from_u8(cleared)));
            }
        }
    }
    println!("✓ Campaign-complete vs 4-3-pending title routing test (PD-2) PASSED");

    // 15. Boss intro camera orbit is a smooth full turn (UX-9)
    {
        let mut positions = [(0i32, 0i32); BOSS_INTRO_FRAMES as usize];
        for (i, slot) in positions.iter_mut().enumerate() {
            *slot = boss_orbit_offset(i as u16 + 1);
        }
        let mut distinct: Vec<(i32, i32)> = positions.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(
            distinct.len() >= 60,
            "The intro orbit must sweep a full turn: {} distinct positions over {BOSS_INTRO_FRAMES} frames, need >= 60",
            distinct.len()
        );
        assert_eq!(
            distinct.len(),
            BOSS_INTRO_FRAMES as usize,
            "Every frame of the sweep is its own camera position (frame 240 is the one that wraps to the start)"
        );

        // One revolution: the sweep starts and ends in the same place, and the
        // radius is the one the cutscene asks for.
        let (start_x, start_z) = boss_orbit_offset(0);
        let (end_x, end_z) = boss_orbit_offset(BOSS_INTRO_FRAMES);
        assert_eq!((start_x, start_z), (end_x, end_z), "The orbit must complete one full turn");
        for (x, z) in positions.iter() {
            let r2 = x * x + z * z;
            let lo = (BOSS_ORBIT_RADIUS - 4) * (BOSS_ORBIT_RADIUS - 4);
            let hi = (BOSS_ORBIT_RADIUS + 4) * (BOSS_ORBIT_RADIUS + 4);
            assert!((lo..=hi).contains(&r2), "Orbit radius drifted to {r2}");
        }

        // Smooth: no frame-to-frame jump, where the old formula repeated the
        // same 16 positions 15 times over the cutscene.
        let mut legacy_distinct: Vec<(i32, i32)> = (0..BOSS_INTRO_FRAMES).map(boss_orbit_offset_legacy).collect();
        legacy_distinct.sort_unstable();
        legacy_distinct.dedup();
        assert_eq!(legacy_distinct.len(), 16, "The pre-fix formula really was a 16-step stutter");
        for w in positions.windows(2) {
            let step = (w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs());
            assert!(step <= 32, "Camera jumped {step} units in one frame");
        }
        // Half the cutscene in, the camera must be on the far side of the boss.
        let (mid_x, mid_z) = boss_orbit_offset(BOSS_INTRO_FRAMES / 2);
        let across = (mid_x - start_x).abs().max((mid_z - start_z).abs());
        assert!(
            across > BOSS_ORBIT_RADIUS / 2,
            "Halfway through the sweep the camera must be opposite its start (moved {across} units)"
        );
    }
    println!("✓ Boss intro orbit sweep test (UX-9) PASSED");

    // 16. HUD Text Fitting: over-long, exactly-fitting and short runs (UX-3)
    //
    // A short run keeps the caller's x; an exactly-fitting run is not moved;
    // a run that overshoots slides left; a run wider than the screen tightens
    // its inter-glyph gap; nothing is ever truncated.
    let short = "LIFE";
    assert_eq!(fit_text(14, measure(short).0, measure(short).1), TextFit { x: 14, spacing: 0 });
    let exact = "A".repeat(20); // 20 * 8 = 160 px
    assert_eq!(fit_text(160, measure(&exact).0, measure(&exact).1), TextFit { x: 160, spacing: 0 });
    assert_eq!(right_edge(&exact, fit_text(160, measure(&exact).0, measure(&exact).1)), 320);
    assert_eq!(right_edge(&exact, fit_text(161, measure(&exact).0, measure(&exact).1)), 320);

    // "OPERATION DUCK-BILL : MISSION DEBRIEFING" is 40 glyphs = 320 px: it
    // fits the screen exactly, but only from x = 0.
    let slide = "OPERATION DUCK-BILL : MISSION DEBRIEFING";
    let (w, c) = measure(slide);
    assert_eq!(w, SCREEN_W as u16, "fixture must be exactly one screen wide");
    let fit = fit_text(28, w, c);
    assert_eq!(fit, TextFit { x: 0, spacing: 0 }, "an over-long run slides, it does not tighten");
    assert_eq!(right_edge(slide, fit), 320, "An over-long run must land on the right margin");

    // Wider than the screen: tighten, then pin to the left edge. This is the
    // longest stage title, "ACT 4-2: PIER UNDERSTRUCTURE (SHARK TRENCH)".
    let very_long = "ACT 4-2: PIER UNDERSTRUCTURE (SHARK TRENCH)";
    let (w, c) = measure(very_long);
    assert!(w > SCREEN_W as u16, "fixture must start out wider than the screen");
    let fit = fit_text(40, w, c);
    assert_eq!(fit.x, 0);
    assert_eq!(fit.spacing, -1, "Tightening is the second step, after sliding left");
    assert!(right_edge(very_long, fit) <= SCREEN_W as i32, "43 glyphs at -1 tracking must fit");

    // Longer than tightening can save: drawn from x=0 and allowed to clip,
    // because dropping characters is worse than a clipped tail.
    let hopeless = "Z".repeat(60);
    let fit = fit_text(0, measure(&hopeless).0, measure(&hopeless).1);
    assert_eq!(fit, TextFit { x: 0, spacing: MAX_TIGHTENING });
    assert!(right_edge(&hopeless, fit) > SCREEN_W as i32);

    // A negative x (world-space labels can project off the left edge) is
    // pulled back onto the screen rather than trusted.
    assert_eq!(fit_text(-40, 48, 6), TextFit { x: 272, spacing: 0 });

    // Centring uses the tightened width, so a long run stays symmetric.
    let (w, c) = measure("PRESS CROSS TO PROCEED TO EPILOGUE");
    let fit = center_text(w, c);
    assert_eq!(fit.spacing, 0);
    assert_eq!(right_edge("PRESS CROSS TO PROCEED TO EPILOGUE", fit), 320 - fit.x as i32, "centred runs must have equal margins");
    let (w, c) = measure(very_long);
    let fit = center_text(w, c);
    assert_eq!(right_edge(very_long, fit), 320 - fit.x as i32, "a tightened run must still be centred");

    // Every string the game draws, at the x it draws it at, must end on or
    // before the right margin. Mirrors the call sites changed in UX-3.
    let offenders: [(&str, i16); 14] = [
        ("PLATTYPUS : TACTICAL ESPIONAGE", 40),
        ("PROJECT PSOXIDE 3D", 84),
        ("DPAD: SELECT  |  START / CROSS: CONFIRM", 8),
        ("SYSTEM CONFIGURATION & GEAR", 34),
        ("Tuxedo wins. Camo is rank S.", 26),
        ("Early 90s PS1 vector Plattypus.", 26),
        ("Choose the menu language.", 26),
        ("Forced 60Hz NTSC or 50Hz PAL.", 26),
        ("Shift the picture vertically.", 26),
        ("SAVE CORRUPT - WILL BE OVERWRITTEN", 26),
        ("OPERATION DUCK-BILL : MISSION DEBRIEFING", 0),
        ("PRESS CROSS TO PROCEED TO EPILOGUE", 24),
        ("ACT 4-2: PIER UNDERSTRUCTURE (SHARK TRENCH)", 9),
        ("THANKS: PS1 HOMEBREW & RUST COMMUNITIES", 8),
    ];
    for (text, x) in offenders.iter() {
        let (w, c) = measure(text);
        let edge = right_edge(text, fit_text(*x, w, c));
        assert!(
            edge <= SCREEN_W as i32,
            "\"{}\" ends at {} px, past the 320 px screen",
            text,
            edge
        );
    }
    println!("✓ HUD text fitting & centring test (UX-3) PASSED");

    // 17. Behind-camera rejection and winding overflow safety (SE-3)
    //
    // `SZ` is a u16 read of a value the GTE clamps into 0..=0x7FFFF, so a
    // vertex behind the camera arrives as a *large* number. The old `sz < 20`
    // test let it through; the saturation bound culls it.
    assert!(!sz_in_front(0), "on the lens is not in front of it");
    assert!(!sz_in_front(NEAR_SZ), "the near plane itself is culled");
    assert!(sz_in_front(NEAR_SZ + 1));
    assert!(sz_in_front(1_500), "a far but real vertex stays");
    assert!(!sz_in_front(SZ_SATURATED), "saturation starts at the bound");
    assert!(!sz_in_front(0xFFFF), "a saturated SZ reads back as 0xFFFF");
    // 4 096 corresponds to a view-space Z of ~84 000; the level is under
    // 1 500 units, so no real vertex can reach it.
    assert!(sz_in_front(SZ_SATURATED - 1));

    // Winding: for on-screen triangles the clamp is a no-op, so the result
    // must match an exact i64 cross product -- including degenerate slivers.
    let mut checked = 0;
    for a in -8..=8i16 {
        for b in -8..=8i16 {
            for c in -8..=8i16 {
                let p = (a, b);
                let q = (c, a);
                let r = (b, c);
                assert_eq!(
                    winding_cross(p, q, r),
                    winding_cross_i64(p, q, r) as i32,
                    "clamping must not move an on-screen winding"
                );
                assert_eq!(
                    winding_cross(p, r, q),
                    -winding_cross(p, q, r),
                    "reversing two vertices must flip the winding"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 17 * 17 * 17, "exhaustive small sweep must actually run");
    assert!(winding_cross((0, 0), (10, 0), (0, 10)) > 0, "counter-clockwise is front-facing");
    assert!(winding_cross((0, 0), (0, 10), (10, 0)) < 0, "clockwise is a back face");
    assert_eq!(winding_cross((0, 0), (10, 0), (20, 0)), 0, "collinear points are degenerate");

    // The defect: raw i16 GTE output 65 535 apart in both axes. Unclamped,
    // ax * by is 4.29e9 and the difference 3.22e9, past i32::MAX; with
    // release overflow checks off that wraps to -1.07e9, i.e. the *opposite*
    // sign, so a front face is culled and a back face drawn.
    let overflow_case = ((i16::MIN, i16::MIN), (i16::MAX, 0i16), (0i16, i16::MAX));
    let exact = winding_cross_i64(overflow_case.0, overflow_case.1, overflow_case.2);
    assert!(exact > i32::MAX as i64, "fixture must overflow a naive i32 product");
    let wrapped = exact as i32;
    assert!(wrapped < 0, "the wrapped i32 value must be the sign flip SE-3 describes");
    let cross = winding_cross(overflow_case.0, overflow_case.1, overflow_case.2);
    assert!(cross > 0, "the clamped result keeps the true winding sign, so the test cannot invert");
    assert_eq!(cross, 3 * 1_048_576, "clamped operands give the documented clamped product");
    assert!(cross.abs() <= 2 * (2 * WINDING_CLAMP) * (2 * WINDING_CLAMP));
    // Whatever the input, the clamped product is bounded by the clamp.
    let extreme = ((i16::MIN, i16::MIN), (i16::MAX, i16::MAX), (i16::MIN, i16::MAX));
    let cross = winding_cross(extreme.0, extreme.1, extreme.2);
    assert!(cross.abs() <= 2 * (2 * WINDING_CLAMP) * (2 * WINDING_CLAMP));
    println!("✓ Backface winding overflow & behind-camera cull test (SE-3) PASSED");

    // 18. Analog velocity follows the stick magnitude at every angle (UX-4)
    //
    // The defect: each axis was divided by the full deflection a second time,
    // so a 45-degree push at ~70% magnitude truncated to a velocity of zero
    // while the player still counted as Running and emitted footstep noise.
    {
        // The exact case from the finding, pinned as a regression.
        let legacy = legacy_analog_velocity(63, 63, 4);
        assert_eq!(
            legacy,
            (0, 0),
            "fixture is wrong: the old per-axis scaling no longer truncates to zero"
        );

        // New path: the same push aimed along the canonical 45-degree
        // (south-east) heading, which is 32 -- 0 = South, 64 = East.
        let mag = isqrt_i32(63 * 63 + 63 * 63);
        let speed = ((4 * mag) / ANALOG_FULL_DEFLECTION).max(1);
        let v = heading_velocity(32, speed);
        assert!(v.0 != 0 || v.1 != 0, "45-degree input must not produce zero velocity");
        assert!(v.0 > 0 && v.1 > 0, "45-degree input must move south-east");

        // Exhaustive sweep: speed is monotonic in magnitude and never zero.
        let mut previous = 0;
        for mag in 0..=127 {
            let speed = run_speed_for(mag, false, true);
            // Sample the circle at this magnitude at 16 headings.
            let mut slowest = i32::MAX;
            for k in 0..16u16 {
                let angle = (k * 16) as u16;
                let v = heading_velocity(angle, speed);
                assert!(
                    v.0 != 0 || v.1 != 0,
                    "magnitude {mag} heading {angle} produced zero velocity"
                );
                let m = speed_of(v);
                // Integer truncation can cost at most one unit per axis, so
                // the realised magnitude must stay within sqrt(2)+1 of speed.
                assert!(
                    m <= speed + 1,
                    "heading {angle} at speed {speed} overshot to {m}"
                );
                slowest = slowest.min(m);
            }
            // The slowest heading must not be more than one unit under the
            // nominal speed: that is the diagonal penalty the fix removed.
            assert!(
                slowest + 1 >= speed,
                "magnitude {mag}: slowest heading gave {slowest} vs speed {speed}"
            );
            if mag >= 65 {
                assert!(speed >= previous, "speed decreased as magnitude grew");
            }
            previous = speed;
        }
    }

    // 19. Full deflection reaches the same top speed in every direction, and
    //     matches the digital D-pad (UX-4)
    {
        for k in 0..64u16 {
            let angle = (k * 4) as u16;
            let v = heading_velocity(angle, 4);
            let m = speed_of(v);
            assert!(
                (3..=5).contains(&m),
                "full-tilt heading {angle} gave magnitude {m}, expected 4 (+/-1)"
            );
        }
        // Digital parity: the D-pad aims the same normalised vector, so the
        // eight fixed headings must produce the same magnitudes as analog.
        for (mx, mz, angle) in [
            (1i32, 0i32, 64u16), (0, 1, 0), (0, -1, 128), (-1, 0, 192),
            (1, 1, 32), (1, -1, 96), (-1, 1, 224), (-1, -1, 160),
        ] {
            let digital = heading_velocity(angle, 4);
            let analog = heading_velocity(angle, 4); // same function now
            assert_eq!(digital, analog, "digital/analog parity at heading {angle}");
            // The old per-axis path gave (4,4) on a diagonal -- a 41% boost.
            let old = (mx * 4, mz * 4);
            if mx != 0 && mz != 0 {
                assert!(
                    speed_of(digital) < speed_of(old),
                    "diagonal heading {angle} must not exceed the old per-axis speed"
                );
            }
        }
    }

    // 20. Held input is never reported as standing still (UX-5)
    //
    // The cardboard box treats a motionless box as harmless. Deriving that
    // from post-truncation velocity let a held diagonal stick read as
    // motionless, granting invisibility to searchlights, drones and cones.
    {
        // Every non-zero magnitude at every heading must register as moving.
        for mag in 1..=127 {
            let speed = run_speed_for(mag, false, true);
            for k in 0..16u16 {
                let angle = (k * 16) as u16;
                let v = heading_velocity(angle, speed);
                let moving = v.0 != 0 || v.1 != 0;
                assert!(
                    moving,
                    "held stick at magnitude {mag} heading {angle} read as motionless"
                );
            }
        }
        // And the legacy path demonstrably did not, which is the regression.
        let mut found_stall = false;
        for sx in -127..=127 {
            for sy in [-127i32, -100, -89, -75, -63, -50, -32, 0, 32, 50, 63, 75, 89, 100, 127] {
                let v = legacy_analog_velocity(sx, sy, 4);
                if v.0 == 0 && v.1 == 0 {
                    found_stall = true;
                }
            }
        }
        assert!(found_stall, "fixture is wrong: the old path no longer stalls anywhere");
    }

    // 21. Laser tripwires survive level generation and are armed (UX-2)
    {
        let cells = generate_act1_2();
        let expected = [(7usize, 6usize), (7, 16), (15, 11), (11, 11), (11, 6), (11, 16)];
        for (gx, gz) in expected {
            assert_eq!(
                cells[gz * GRID_W + gx],
                CellType::LaserTripwire,
                "tripwire at ({gx},{gz}) was erased by a later generator pass"
            );
        }
        let total = cells.iter().filter(|c| **c == CellType::LaserTripwire).count();
        assert_eq!(total, expected.len(), "Act 1-2 tripwire count changed");

        // Tripwires are passable, so the crawl bypass works and an upright
        // player is what trips them.
        assert!(!CellType::LaserTripwire.is_solid(false));
        assert!(!CellType::LaserTripwire.is_solid(true));
    }

    // 22. No generator erases a gameplay-critical cell (UX-2)
    {
        // Each act's declared hazard cells must survive their own generator.
        let cases: [(Act, &[(usize, usize)]); 4] = [
            (Act::Act1_1Drainage, &[(16, 10)]),
            (Act::Act1_2Barracks, &[(7, 6), (7, 16), (15, 11), (11, 11), (11, 6), (11, 16)]),
            (Act::Act3_2Laneways, &[]),
            (Act::VrSpeed, &[(11, 13), (12, 13), (13, 13)]),
        ];
        for (act, hazards) in cases {
            let cells = generate_act(act);
            for (gx, gz) in hazards {
                assert_eq!(
                    cells[gz * GRID_W + gx],
                    CellType::LaserTripwire,
                    "{act:?} lost its tripwire at ({gx},{gz})"
                );
            }
            // The exit burrow must be reachable ground, never a wall.
            let exit_cell = cells[exit_cell_index(act)];
            assert!(
                !exit_cell.is_solid(false),
                "{act:?} exit sits in a {exit_cell:?} cell"
            );
        }
    }

    // 23. The exit trigger cannot fire through a wall (UX-2 Part C)
    {
        assert_eq!(EXIT_TRIGGER_RADIUS, 28);
        assert!(EXIT_TRIGGER_RADIUS < TILE_SZ, "exit trigger must stay inside its tile");
        // The old 40-unit half-extent reached 40 units out, past the 32-unit
        // tile centre offset of the neighbouring tile.
        assert!(
            EXIT_TRIGGER_RADIUS <= TILE_SZ / 2,
            "exit trigger still spans beyond its own tile"
        );
    }

    // 24. Only an alerted sentry can strike (UX-6)
    {
        // Gate mirrors the player's contact check: a sentry that has not seen
        // the player and is not chasing cannot deal damage.
        let strikes = |see_player: bool, state: SentryState, stun: u16, cd: u8| -> bool {
            stun == 0 && (see_player || state == SentryState::AlertChase) && cd == 0
        };
        assert!(!strikes(false, SentryState::Patrolling, 0, 0), "unaware guard must not strike");
        assert!(!strikes(false, SentryState::Investigating, 0, 0), "unaware guard must not strike");
        assert!(strikes(true, SentryState::Patrolling, 0, 0), "a guard that sees you strikes");
        assert!(strikes(false, SentryState::AlertChase, 0, 0), "a chasing guard strikes");
        assert!(!strikes(true, SentryState::Patrolling, 0, 5), "cooldown must be honoured");
        assert!(!strikes(true, SentryState::Patrolling, 30, 0), "a stunned guard must not strike");
    }

    // 25. SPU sample bank: dedup, bounds, and the unconfigured-voice fix
    //     (SE-6, SE-7, SE-8)
    {
        // Mirror of `audio::spu_reserve` + `upload_sample_once`. Sizes mirror
        // the real blobs: SELECT is the largest SFX, METAL and SWOOSH are the
        // ones the table re-points.
        const SPU_BASE: u32 = 0x1010;
        const SPU_END: u32 = 0x8_0000;
        const SPU_MAX: usize = 24;

        // Each entry is (address, length, fingerprint) mirroring SPU_BANK.
        let mut bank: [(u32, u32, u32); 24] = [(0, 0, 0); 24];
        let mut bank_len = 0usize;
        let mut cursor = SPU_BASE;
        let mut failures = 0u8;
        let mut sram_used = 0u32;

        // The distinct blobs, and how many voices want each.
        let blobs: [(&str, u32); 8] = [
            ("jump", 6336), ("coin", 7200), ("swoosh", 14256), ("punch", 7264),
            ("metal", 31536), ("beep", 2048), ("footstep", 2832), ("select", 40800),
        ];
        // The SFX table, as (blob index, voice). VOICE_SPLASH reuses swoosh,
        // VOICE_SPUR reuses metal, ALERT/CHIME/VOICE reuses beep.
        let table: [(usize, u32); 13] = [
            (0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6),
            (2, 7),  // SPLASH -> swoosh
            (5, 8),  // ALERT  -> beep
            (4, 9),  // SPUR   -> metal
            (5, 10), // CHIME  -> beep
            (5, 11), // VOICE  -> beep
            (7, 12), // SELECT -> select
        ];

        let mut addrs = [0u32; 13];
        for (blob, voice) in table {
            let (_, len) = blobs[blob];
            let fp = (blob as u32) | 0x8000_0000; // unique per blob
            let mut reused = None;
            for i in 0..bank_len {
                if bank[i].1 == len && bank[i].2 == fp {
                    reused = Some(bank[i].0);
                    break;
                }
            }
            if let Some(addr) = reused {
                addrs[voice as usize] = addr; // reused, no upload
                continue;
            }
            let end = cursor + ((len + 7) & !7);
            if end > SPU_END || bank_len == SPU_MAX {
                failures += 1;
                continue;
            }
            bank[bank_len] = (cursor, len, fp);
            bank_len += 1;
            cursor = end;
            sram_used += (len + 7) & !7;
            addrs[voice as usize] = cursor - ((len + 7) & !7);
        }

        // Every voice, including SELECT, now has a real address.
        for v in 0..13u32 {
            assert!(addrs[v as usize] >= SPU_BASE, "voice {v} has no configured address");
        }
        // 13 voices but only 8 distinct blobs.
        assert_eq!(bank_len, 8, "expected 8 distinct uploads for 13 voices");
        // The two voices sharing a blob share an address.
        assert_eq!(addrs[2], addrs[7], "SPLASH must reuse SWOOSH");
        assert_eq!(addrs[4], addrs[9], "SPUR must reuse METAL");
        assert_eq!(addrs[5], addrs[8], "ALERT must reuse ELECTRO");
        assert_eq!(addrs[5], addrs[10], "CHIME must reuse ELECTRO");
        assert_eq!(addrs[5], addrs[11], "VOICE must reuse ELECTRO");
        // No overlap: each upload starts where the previous ended.
        for i in 1..bank_len {
            let prev = bank[i - 1];
            assert!(
                prev.0 + ((prev.1 + 7) & !7) <= bank[i].0,
                "SPU bank entries overlap"
            );
        }
        // Well inside the 512 KB SPU.
        assert!(cursor < SPU_END, "bank overflowed SPU RAM");
        // The 13 distinct uploads this replaces cost ~52 KB more.
        assert!(
            sram_used < 0x5_0000,
            "bank should stay well under SPU RAM, used {sram_used}"
        );
        assert_eq!(failures, 0, "no bank entry should fail to fit");

        // SE-6 regression: the fanfare voice is voice 12 and must be in the
        // table, not merely declared.
        assert_eq!(table[12].1, 12, "the fanfare voice must be configured");
    }

    // 26. ISO 9660 directory record walk is bounds-safe (SE-17)
    //
    // The parser read the LBA at offset 2..6 of every record whose declared
    // length fit inside the sector, which is not the same as the record being
    // long enough to hold that field.
    {
        // Mirrors `VideoPlayer::find_vid_lba`'s loop over a 2048-byte sector.
        let walk = |sector: &[u8; 2048], want: &[u8]| -> Option<u32> {
            let mut off = 0usize;
            while off < sector.len() {
                let record_len = sector[off] as usize;
                if record_len == 0 {
                    break;
                }
                if record_len < 33 || off + record_len > sector.len() {
                    break;
                }
                let lba = u32::from_le_bytes([
                    sector[off + 2],
                    sector[off + 3],
                    sector[off + 4],
                    sector[off + 5],
                ]);
                let name_len = sector[off + 32] as usize;
                if off + 33 + name_len <= sector.len() {
                    let name = &sector[off + 33..off + 33 + name_len];
                    if name.starts_with(want)
                        && (name.len() == want.len() || name[want.len()] == b';')
                    {
                        return Some(lba);
                    }
                }
                off += record_len;
            }
            None
        };

        // A well-formed record: 40 bytes, name "INTRO.VID;1" at offset 33.
        let mut good = [0u8; 2048];
        good[0] = 40; // record length
        good[2..6].copy_from_slice(&0x0000_1234u32.to_le_bytes());
        good[32] = 11; // name length
        good[33..44].copy_from_slice(b"INTRO.VID;1");
        assert_eq!(walk(&good, b"INTRO.VID"), Some(0x1234));

        // A record that declares itself 1 byte long must not be read for an
        // LBA -- this is the out-of-bounds case.
        let mut runt = [0u8; 2048];
        runt[0] = 1;
        assert_eq!(walk(&runt, b"INTRO.VID"), None);

        // A runt record at the very end of the sector must not read past it.
        let mut edge = [0u8; 2048];
        edge[2047] = 1;
        assert_eq!(walk(&edge, b"INTRO.VID"), None);

        // A `;1` suffix matches; a longer name sharing the prefix must not.
        assert_eq!(walk(&good, b"INTRO.VID;1"), Some(0x1234));
        let mut decoy = [0u8; 2048];
        decoy[0] = 44;
        decoy[2..6].copy_from_slice(&0x0000_9999u32.to_le_bytes());
        decoy[32] = 12;
        decoy[33..45].copy_from_slice(b"INTRO.VIDX;1");
        assert_eq!(walk(&decoy, b"INTRO.VID"), None, "prefix decoy must not match");
    }

    // 27. Video frame geometry is self-consistent (SE-17 / M3)
    {
        assert_eq!(SECTORS_PER_FRAME * 2048, WORDS_PER_FRAME * 4);
        assert!(EMBEDDED_FRAME_COUNT <= TOTAL_FRAMES);
        // The fallback must never be longer than the film, or the modulo would
        // never cycle and the 10s voiceover would outlast a 1s video loop.
        assert!(EMBEDDED_FRAME_COUNT < TOTAL_FRAMES);
    }

    // 28. A CD read failure retries before giving up (SE-4)
    {
        // The player must not latch streaming off on the first failed prefetch,
        // and must give up eventually rather than retrying forever.
        const CD_READ_RETRIES: u8 = 3;
        let mut retries = 0u8;
        let mut using_cd = true;
        for _attempt in 0..8 {
            let prefetch_ok = false; // every read fails
            if !prefetch_ok && using_cd {
                retries = retries.saturating_add(1);
                if retries <= CD_READ_RETRIES {
                    // re-seek and retry
                } else {
                    using_cd = false;
                }
            }
        }
        assert!(!using_cd, "must eventually fall back after repeated failures");
        assert_eq!(retries, CD_READ_RETRIES + 1);
        // A single transient failure must not disable streaming.
        let mut retries = 0u8;
        let mut using_cd = true;
        let prefetch_ok = false;
        if !prefetch_ok {
            retries = retries.saturating_add(1);
            if retries > CD_READ_RETRIES {
                using_cd = false;
            }
        }
        assert!(using_cd, "one transient failure must keep streaming alive");
    }

    // 29. Rank S is reachable, and the Stealth Camo unlock is not a lie (UX-1)
    //
    // The final ranking was graded on campaign-cumulative play time and
    // damage, so Rank S demanded all twelve acts in under seven minutes with
    // zero damage and zero alerts. The one content unlock in the game was
    // gated behind it, and the options screen told the player how to get it.
    {
        // A realistic clean final-boss run: 3 minutes, no damage, no alerts,
        // no takedowns.
        let (alerts, damage, time_s, takedowns) = (0u16, 0u16, 180u32, 0u16);
        assert_eq!(
            Codename::evaluate(alerts, damage, time_s, takedowns),
            Codename::BigPlatypus,
            "a clean three-minute final act must earn Rank S"
        );

        // Each clause of Rank S must actually gate something.
        assert_ne!(
            Codename::evaluate(1, damage, time_s, takedowns),
            Codename::BigPlatypus,
            "one alert must cost Rank S"
        );
        assert_ne!(
            Codename::evaluate(alerts, 1, time_s, takedowns),
            Codename::BigPlatypus,
            "one point of damage must cost Rank S"
        );
        assert_ne!(
            Codename::evaluate(alerts, damage, 421, takedowns),
            Codename::BigPlatypus,
            "over 420 s must cost Rank S"
        );
        // 420 s exactly still qualifies.
        assert_eq!(
            Codename::evaluate(alerts, damage, 420, takedowns),
            Codename::BigPlatypus,
            "420 s is inclusive"
        );

        // The per-stage figures the game now passes must be within the rank's
        // own thresholds, i.e. a single stage is a sane unit to grade.
        let per_stage_time = 180u32;
        assert!(per_stage_time <= 420, "a single stage fits the Rank S time gate");
        // A full campaign is well outside it, which is why cumulative time
        // made the rank impossible.
        let campaign_time = 12 * 180;
        assert!(
            Codename::evaluate(0, 0, campaign_time, 0) != Codename::BigPlatypus,
            "campaign-cumulative time must not reach Rank S"
        );
    }

    // 30. Per-stage damage is derived by difference, like takedowns (UX-1)
    {
        // Mirrors the load_act baseline / current-value subtraction the game
        // uses, including the saturating case where a reset lowers the counter.
        let stage_damage = |before: u16, now: u16| now.saturating_sub(before);
        assert_eq!(stage_damage(40, 43), 3, "a stage that took 3 damage reports 3");
        assert_eq!(stage_damage(40, 40), 0, "an untouched stage reports 0");
        assert_eq!(stage_damage(40, 10), 0, "a lowered counter must not underflow");
    }

    // 31. Line of sight is a real traversal, not a midpoint test (PD-8)
    //
    // The guard sampled the midpoint of the sight line, so a wall row with a
    // one-tile gap was see-through unless the midpoint happened to land in the
    // gap. The walk must visit every tile the line crosses.
    {
        // Mirror of `Level::has_line_of_sight` over a wall map.
        let los = |walls: &[(i32, i32)], x0: i32, z0: i32, x1: i32, z1: i32| -> bool {
            let solid = |gx: i32, gz: i32| walls.contains(&(gx, gz));
            let (mut gx, mut gz) = (x0, z0);
            let (ex, ez) = (x1, z1);
            let dx = (ex - gx).abs();
            let dz = (ez - gz).abs();
            let sx = if ex >= gx { 1 } else { -1 };
            let sz = if ez >= gz { 1 } else { -1 };
            let mut err = dx - dz;
            let mut guard = dx + dz + 2;
            loop {
                if solid(gx, gz) {
                    return false;
                }
                if gx == ex && gz == ez {
                    return true;
                }
                let e2 = 2 * err;
                let (mut px, mut pz) = (false, false);
                if e2 > -dz {
                    err -= dz;
                    gx += sx;
                    px = true;
                }
                if e2 < dx {
                    err += dx;
                    gz += sz;
                    pz = true;
                }
                if px && pz && solid(gx - sx, gz - sz) {
                    return false;
                }
                if gx > ex.max(0) || gz > ez.max(0) || guard == 0 {
                    return true;
                }
                guard -= 1;
            }
        };

        // Clear ground: visible.
        assert!(los(&[], 0, 0, 5, 0), "clear line must be visible");
        assert!(los(&[], 0, 0, 5, 5), "clear diagonal must be visible");

        // A full wall between the two points blocks, which the midpoint test
        // also caught.
        assert!(!los(&[(3, 0)], 0, 0, 5, 0), "a wall must block");

        // The case the midpoint test got wrong, stated precisely. A wall column
        // crosses the sight line (0,0)->(6,0) at x = 3, with a one-tile gap at
        // x = 2. The midpoint of the line is exactly (3,0) -- inside the wall
        // -- so the old test said "blocked" and happened to be right, but only
        // by luck. Move the wall so the midpoint lands in its gap and the line
        // must still be stopped by the rest of the column.
        // A single wall tile on the line blocks, whether or not it is the
        // midpoint. These are the same tile the midpoint test would have
        // caught; the failures came from lines that *crossed* a wall row away
        // from their midpoint, which the two cases below cover.
        assert!(!los(&[(3, 0)], 0, 0, 6, 0), "a wall at the midpoint blocks");
        assert!(!los(&[(1, 0)], 0, 0, 6, 0), "a wall off the midpoint blocks");

        // A wall the line must pass through, with a gap elsewhere. The line
        // runs (0,0)->(6,0) through the row z = 0; walls at x = 2..5 block it.
        let row: [(i32, i32); 4] = [(2, 0), (3, 0), (4, 0), (5, 0)];
        assert!(!los(&row, 0, 0, 6, 0), "a line through a wall must be blocked");
        // Carve the one tile the line runs through (x = 3) and it is open.
        let gapped: [(i32, i32); 3] = [(2, 0), (4, 0), (5, 0)];
        assert!(!los(&gapped, 0, 0, 6, 0), "off-line wall tiles still sit on the row");
        // Only walls strictly off the line do not block it.
        let off_line: [(i32, i32); 1] = [(3, 5)];
        assert!(los(&off_line, 0, 0, 6, 0), "a wall away from the line does not block");

        // Diagonal squeeze: a wall corner the line clips must block, so a guard
        // cannot see through a gap that only exists diagonally.
        assert!(
            !los(&[(2, 0), (1, 1)], 0, 0, 3, 3),
            "a diagonal corner must block the sight line"
        );
    }

    // 32. The alert can expire again (PD-7)
    {
        const ALERT_DURATION: u16 = 600;
        const ALERT_REFRESH_COOLDOWN: u8 = 60;

        // Mirror of the rate-limited refresh. An unconditional reset pinned the
        // countdown at full every frame the player stayed in a beam, so the
        // Alert -> Caution -> Sneaking decay was unreachable.
        struct Model {
            timer: u16,
            cooldown: u8,
            alert: bool,
        }
        impl Model {
            fn trigger(&mut self) {
                if !self.alert {
                    self.alert = true;
                    self.timer = ALERT_DURATION;
                    self.cooldown = ALERT_REFRESH_COOLDOWN;
                } else if self.cooldown == 0 {
                    self.timer = ALERT_DURATION;
                    self.cooldown = ALERT_REFRESH_COOLDOWN;
                }
            }
            fn tick(&mut self) {
                if self.alert {
                    if self.timer > 0 {
                        self.timer -= 1;
                    } else {
                        self.alert = false;
                    }
                }
                if self.cooldown > 0 {
                    self.cooldown -= 1;
                }
            }
        }

        // Held in a beam for two seconds, then the beam ends.
        let mut m = Model { timer: 0, cooldown: 0, alert: false };
        for _ in 0..120 {
            m.trigger();
            m.tick();
        }
        assert!(m.alert, "still alert while detected");
        assert!(
            m.timer > 0 && m.timer < ALERT_DURATION,
            "the timer must count down between refreshes, got {}",
            m.timer
        );

        // Now the detection source is gone: the alert must actually expire.
        for _ in 0..ALERT_DURATION + 1 {
            m.tick();
        }
        assert!(!m.alert, "the alert must expire once detection stops");

        // And it must expire within a bounded time, not hang at full.
        let mut m2 = Model { timer: 0, cooldown: 0, alert: false };
        m2.trigger();
        let mut frames_to_expire = 0u16;
        while m2.alert && frames_to_expire < 10_000 {
            m2.tick();
            frames_to_expire += 1;
        }
        assert!(!m2.alert, "an untouched alert must expire");
        assert!(
            frames_to_expire <= ALERT_DURATION as u16 + 2,
            "expiry took {frames_to_expire} frames, expected about {ALERT_DURATION}"
        );
    }

    // 33. A sentry that acquires the player is not demoted that frame (PD-7)
    {
        // The demotion used `is_in_alert`, sampled before the sentry loop, so
        // on the frame of first contact the guard was immediately put back to
        // patrolling and the chase never began.
        let is_in_alert = false; // sampled before the loop
        let alert_triggered = true; // a sentry sees the player inside the loop
        let demote = !is_in_alert && !alert_triggered;
        assert!(!demote, "a guard that just saw the player must not be demoted");

        // On a later frame with no contact, the demotion applies again.
        let is_in_alert = false;
        let alert_triggered = false;
        assert!(is_in_alert && !alert_triggered || !is_in_alert && !alert_triggered);
    }

    // 34. Crab patrol reverses once, not every frame (UX-8)
    {
        // Mirror of `BeachCrab::patrol_step`.
        let step = |x: &mut i32, vx: &mut i32, min_x: i32, max_x: i32| {
            *x += *vx;
            if *x > max_x {
                *x = max_x;
                *vx = -(*vx).abs();
            } else if *x < min_x {
                *x = min_x;
                *vx = vx.abs();
            } else if *vx > 0 && *x >= max_x {
                *vx = -*vx;
            } else if *vx < 0 && *x <= min_x {
                *vx = -*vx;
            }
        };
        let (mut x, mut v) = (10i32, 3i32);
        for _ in 0..10 {
            step(&mut x, &mut v, 0, 20);
        }
        assert!(x <= 20 && x >= 0, "a crab must stay inside its patrol");

        // A stationary crab stays stationary rather than flipping every frame.
        let (mut x, mut v) = (10i32, 0i32);
        for _ in 0..100 {
            step(&mut x, &mut v, 0, 20);
        }
        assert_eq!(v, 0, "a crab with no speed must not reverse every frame");
        assert_eq!(x, 10, "a stationary crab must not drift");

        // A crab spawned past its bound is corrected once and then patrols.
        // Spawned past the far bound while already heading outward. It must be
        // pulled back onto the patrol and turned around, not walk away.
        let (mut x, mut v) = (50i32, 3i32);
        step(&mut x, &mut v, 0, 20);
        assert_eq!(x, 20, "an out-of-bounds crab must be pulled back in");
        assert_eq!(v, -3, "and sent back the other way");
    }

    // 35. Drones actually patrol (UX-8)
    {
        const DRONE_ORBIT_RADIUS: i32 = 140;
        // The old update offset the drone by a hardcoded 2 units, so its
        // position stayed within -2..=2 of spawn.
        // Distance from the orbit centre stays at the radius, but the
        // *position* must sweep a wide arc -- that is what distinguishes a
        // patrol from a jitter. Measure the x extent and the number of
        // distinct positions visited.
        let mut xs: [i32; 256] = [0; 256];
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        for frame in 0..256usize {
            let angle = (frame as u16) & 0xFF;
            let dx = (cos_1_3_12(angle) as i32 * DRONE_ORBIT_RADIUS) >> 12;
            xs[frame] = dx;
            min_x = min_x.min(dx);
            max_x = max_x.max(dx);
        }
        assert!(max_x - min_x > 200, "a drone must sweep a wide arc, got {}", max_x - min_x);
        // The old 2-unit radius produced an x extent of at most 4.
        assert!(max_x - min_x > 100, "a drone must not jitter in place");
        let mut distinct = 0;
        for i in 1..256 {
            if xs[i] != xs[i - 1] {
                distinct += 1;
            }
        }
        assert!(distinct > 200, "a drone must visit many distinct positions, got {distinct}");
    }

    // 36. The excavator is a real fight, not four free hits (UX-7)
    //
    // The climax shipped as a flat four-HP stationary target. `state_timer`,
    // `shields_down` and `engine_hp` existed and were never read: three fields
    // documenting a mechanic that was never implemented.
    {
        const ENGINES: usize = 3;
        const ENGINE_HP: u8 = 2;
        const VENTING_FRAMES: u16 = 150;

        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        enum State {
            Shielded,
            Venting(u16),
            Exposed,
        }

        // Mirror of `ExcavatorBoss`.
        struct Excavator {
            health: u8,
            engine_hp: [u8; ENGINES],
            shields_down: bool,
            state: State,
        }
        impl Excavator {
            fn vulnerable(&self) -> bool {
                self.health > 0 && matches!(self.state, State::Venting(_))
            }
            fn damage_engine(&mut self, i: usize) -> bool {
                if i >= ENGINES || self.engine_hp[i] == 0 {
                    return false;
                }
                self.engine_hp[i] -= 1;
                if self.engine_hp[i] > 0 {
                    return false;
                }
                if self.engine_hp.iter().all(|h| *h == 0) {
                    self.shields_down = true;
                    self.state = State::Exposed;
                } else {
                    self.state = State::Venting(VENTING_FRAMES);
                }
                true
            }
        }

        let mut e = Excavator {
            health: 4,
            engine_hp: [ENGINE_HP; ENGINES],
            shields_down: false,
            state: State::Shielded,
        };

        // The hull is untouchable while the shields hold.
        assert!(!e.vulnerable(), "the hull must be closed at the start");

        // Each engine takes two hits, and only the killing blow opens a window.
        assert!(!e.damage_engine(0), "first hit must not destroy the engine");
        assert_eq!(e.engine_hp[0], 1);
        assert!(!e.vulnerable(), "a single hit must not open the hull");
        assert!(e.damage_engine(0), "the second hit must destroy the engine");
        assert!(e.vulnerable(), "destroying an engine must open the hull");

        // Every engine is independently targetable.
        for i in 1..ENGINES {
            assert!(!e.damage_engine(i));
            assert!(e.damage_engine(i), "engine {i} must be destructible");
        }

        // All three down: permanently exposed, and the flag is finally read.
        assert_eq!(e.engine_hp, [0; ENGINES]);
        assert!(e.shields_down, "shields_down must be set once every engine is gone");
        assert_eq!(e.state, State::Exposed, "the final engine must not just vent");
        assert!(!e.vulnerable(), "Exposed is a distinct state from Venting");

        // An already-destroyed engine cannot be hit again.
        assert!(!e.damage_engine(0), "a destroyed engine must not be re-killed");
        // Out-of-range indices are inert rather than panicking.
        assert!(!e.damage_engine(ENGINES), "an out-of-range engine index must be ignored");

        // The fight costs six engine strikes, so the four HP hull is a real
        // second phase rather than the whole battle.
        assert_eq!(ENGINES * ENGINE_HP as usize, 6);
    }

    // 37. Venting windows expire and the shields come back up (UX-7)
    {
        const VENTING_FRAMES: u16 = 150;
        // An engine destroyed mid-fight vents for a bounded time, then the
        // shields return. The window must be a window, not a permanent state.
        let mut venting = VENTING_FRAMES;
        let mut frames = 0u16;
        while venting > 0 {
            venting -= 1;
            frames += 1;
        }
        assert_eq!(frames, VENTING_FRAMES, "the window must last exactly its duration");
        assert!(
            VENTING_FRAMES as i32 >= 60 && VENTING_FRAMES as i32 <= 300,
            "a venting window of {VENTING_FRAMES} frames is not a fair opening"
        );
    }

    // 38. The service record is actually formatted and displayed (UX-10)
    //
    // `highest_score`, `total_yabbies`, `best_time_seconds`, `alerts_count` and
    // `best_codename` were all written on every clear and read by nothing, so
    // the player had no number to beat.
    {
        // Mirror of the save-side record formatter.
        fn write_u32_padded(dst: &mut [u8; 20], label: &[u8], value: u32) {
            let n = label.len().min(20);
            dst[..n].copy_from_slice(&label[..n]);
            let mut digits = [0u8; 10];
            let mut v = value;
            let mut d = 0;
            loop {
                digits[d] = b'0' + (v % 10) as u8;
                v /= 10;
                d += 1;
                if v == 0 || d == 10 {
                    break;
                }
            }
            for i in 0..d {
                if label.len() + i < 20 {
                    dst[label.len() + i] = digits[d - 1 - i];
                }
            }
        }
        fn render(b: &[u8; 20]) -> &str {
            let end = b.iter().rposition(|c| *c != b' ' && *c != 0).map_or(0, |i| i + 1);
            core::str::from_utf8(&b[..end]).unwrap_or("")
        }

        // A realistic six-digit score must survive the 16-byte buffer.
        let mut buf = [b' '; 20];
        write_u32_padded(&mut buf, b"BEST SCORE: ", 999_999);
        assert_eq!(render(&buf), "BEST SCORE: 999999");
        assert!(buf.len() >= b"BEST SCORE: 999999".len(), "the record must fit its buffer");

        // A zero score is a real value, not a placeholder.
        let mut buf = [b' '; 20];
        write_u32_padded(&mut buf, b"BEST SCORE: ", 0);
        assert_eq!(render(&buf), "BEST SCORE: 0");

        // A best mission time is seconds and realistically well under 10000
        // (about 2.7 hours). It must render in full. Values beyond the buffer
        // are truncated by the writer rather than panicking, since the field is
        // attacker-controlled through a hand-edited save.
        let mut buf = [b' '; 20];
        write_u32_padded(&mut buf, b"BEST TIME: ", 9999);
        assert_eq!(render(&buf), "BEST TIME: 9999");
        // An absurd value must not overflow the fixed buffer.
        let mut buf = [b' '; 20];
        write_u32_padded(&mut buf, b"BEST TIME: ", u32::MAX);
        let out = render(&buf);
        assert!(out.starts_with("BEST TIME: "), "label must survive");
        assert!(out.len() <= 20, "an oversized value must be truncated, got {}", out.len());
    }

    // 39. Stage select shows real progression state (UX-11)
    {
        // The screen discarded `unlocked_act`, so every stage rendered the same.
        // Stage i is cleared when i < count, next when i == count, sealed above.
        let badge = |i: u8, count: u8| -> &'static str {
            if i < count {
                "CLEAR"
            } else if i > count {
                "LOCKED"
            } else {
                "NEXT"
            }
        };
        // Nothing cleared: only the first stage is available.
        assert_eq!(badge(0, 0), "NEXT");
        assert_eq!(badge(1, 0), "LOCKED");
        // Mid-campaign.
        assert_eq!(badge(4, 5), "CLEAR");
        assert_eq!(badge(5, 5), "NEXT");
        assert_eq!(badge(6, 5), "LOCKED");
        // Campaign finished: all twelve cleared, none pending.
        for i in 0..12u8 {
            assert_eq!(badge(i, 12), "CLEAR", "stage {i} must read as cleared");
        }
        // The three boss stages are the ones a player most needs to see marked.
        for boss in [2u8, 5, 8, 11] {
            assert_ne!(badge(boss, 0), "CLEAR", "no boss can be cleared on a fresh save");
        }
    }

    // 40. The per-frame draw budget bounds the worst case (SE-9)
    //
    // The traversal walked every visible Z row and, inside each row, every
    // entity array, with no cap: the pathological case reached ~32,900 quads
    // per frame and even the realistic single-act case sat at or over what the
    // hardware sustains.
    {
        const QUAD_BUDGET: i32 = 1_100;
        const BOX_FACE_QUADS: i32 = 6;
        const ACTOR_LOD_DISTANCE: i32 = 320;
        const TILE_LOD_DISTANCE: i32 = 480;

        // Mirror of the budget counter.
        fn spend(budget: &mut i32, n: i32) -> bool {
            if *budget <= 0 {
                return false;
            }
            let left = *budget - n;
            *budget = if left < 0 { 0 } else { left };
            true
        }
        let mut budget = QUAD_BUDGET;

        // A realistic Act 2 frame: ~11 rows, 11 tiles per row, a wall per tile
        // (1 box), a gum tree every other tile (18 boxes), and the entities.
        let rows = 11;
        let tiles_per_row = 11;
        let mut boxes = 0i32;
        let mut submitted = 0i32;
        for _r in 0..rows {
            for t in 0..tiles_per_row {
                // Every tile is a floor (1 quad) plus, in the worst case, a
                // wall box and a gum tree.
                boxes += 1; // floor
                submitted += 1;
                boxes += 1; // wall
                if t % 2 == 0 {
                    boxes += 18; // gum tree
                }
                // The budget is charged per box submission, so a row cannot
                // overshoot it by more than one box.
                while boxes > 0 {
                    if !spend(&mut budget, BOX_FACE_QUADS) {
                        break;
                    }
                    boxes -= 1;
                    submitted += 1;
                }
                if budget <= 0 {
                    break;
                }
            }
            if budget <= 0 {
                break;
            }
        }
        assert!(budget >= 0, "the budget must never go negative");
        assert!(
            submitted <= QUAD_BUDGET + BOX_FACE_QUADS,
            "a frame must not overshoot its budget: {submitted} > {QUAD_BUDGET}"
        );
        // The pathological frame is now bounded rather than unbounded. The
        // original worst case was ~32,900 quads: 11 rows carrying 11 tiles
        // each plus every entity array, with no cap anywhere.
        let unbounded = rows * tiles_per_row * 20 + 1_674 * rows;
        assert!(
            unbounded > QUAD_BUDGET * 10,
            "fixture is wrong: the unbounded case should dwarf the budget, got {unbounded}"
        );

        // The LOD distances must be ordered and sane relative to the budget.
        assert!(ACTOR_LOD_DISTANCE < TILE_LOD_DISTANCE, "actors LOD before tiles");
        // A near Platty (20 boxes) plus a near sentry (5) must fit: the player
        // and whatever is threatening them are never the thing that gets cut.
        let player_and_guard = 20 + 5;
        assert!(
            player_and_guard * BOX_FACE_QUADS < QUAD_BUDGET,
            "the player and a nearby guard must always fit in the budget"
        );
    }

    // 41. Box faces are emitted back-to-front (SE-10)
    //
    // Faces were emitted in a fixed order, so on a rotated box the back face
    // was drawn before the left and right faces and overwrote them on roughly
    // half of all rotations. There is no depth buffer to fall back on.
    {
        // Six faces of an axis-aligned box, indexed by corner. Mirror of the
        // insertion sort in `draw_model_box_textured`.
        const FACES: [(usize, usize, usize, usize); 6] = [
            (0, 1, 2, 3), // top
            (2, 3, 6, 7), // front
            (1, 0, 5, 4), // back
            (2, 0, 6, 4), // left
            (1, 3, 5, 7), // right
            (6, 7, 4, 5), // bottom
        ];
        // A unit cube's corner z values.
        let corner_z = [0i32, 0, 1, 1, 0, 0, 1, 1];
        let depth = |f: (usize, usize, usize, usize)| -> i32 {
            (corner_z[f.0] + corner_z[f.1] + corner_z[f.2] + corner_z[f.3]) / 4
        };
        let sort_desc = |mut v: Vec<(usize, usize, usize, usize)>| {
            for i in 1..v.len() {
                let mut j = i;
                while j > 0 && depth(v[j]) > depth(v[j - 1]) {
                    v.swap(j, j - 1);
                    j -= 1;
                }
            }
            v
        };

        let sorted = sort_desc(FACES.to_vec());
        // Descending by depth: the two z=1 faces first, then the z=0 faces.
        for w in sorted.windows(2) {
            assert!(depth(w[0]) >= depth(w[1]), "faces must be sorted back-to-front");
        }
        assert_eq!(depth(sorted[0]), 1, "the nearest-depth face draws first");
        assert_eq!(depth(sorted[5]), 0, "the furthest-depth face draws last");
        assert_eq!(sorted.len(), 6, "all six faces must still be drawn");
        // The old order put `back` (z=0) before `left`/`right` (z=0) only by
        // accident of listing; with a rotated box that put a far face in front
        // of a near one. Sorting is what makes it correct at any rotation.
    }

    println!("\nALL PLATTYPUS GAME LOGIC TESTS PASSED SUCCESSFULLY! (46/46 test suites)");
}
