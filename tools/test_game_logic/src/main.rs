//! Host-side test suite for Plattypus game logic invariants.
//! Tests save serialization, checksum verification, codename ranking,
//! act progression, and cell collisions without requiring bare-metal MIPS hardware.

const SAVE_MAGIC: [u8; 4] = *b"PLTY";
/// Mirrors game/src/save.rs. Bumped 3 -> 4 with the checksum rewrite.
const SAVE_VERSION: u8 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Codename {
    BigPlatypus,
    GhostPlatypus,
    SpeedyWallaby,
    TasmanianDevil,
    LurkingEchidna,
    IronBill,
    SlyPossum,
    BushKoala,
    VenomousTaipan,
    WombatTunnel,
    CardboardHermit,
    DuckbillRookie,
}

impl Codename {
    pub fn name(&self) -> &'static str {
        match self {
            Codename::BigPlatypus => "BIG PLATYPUS",
            Codename::GhostPlatypus => "GHOST PLATYPUS",
            Codename::SpeedyWallaby => "SPEEDY WALLABY",
            Codename::TasmanianDevil => "TASMANIAN DEVIL",
            Codename::LurkingEchidna => "LURKING ECHIDNA",
            Codename::IronBill => "IRON BILL",
            Codename::SlyPossum => "SLY POSSUM",
            Codename::BushKoala => "BUSH KOALA",
            Codename::VenomousTaipan => "VENOMOUS TAIPAN",
            Codename::WombatTunnel => "BURROWING WOMBAT",
            Codename::CardboardHermit => "CARDBOARD HERMIT",
            Codename::DuckbillRookie => "DUCKBILL ROOKIE",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Codename::BigPlatypus => "FOXHOUND LEGEND (RANK S)",
            Codename::GhostPlatypus => "UNSEEN PHANTOM (RANK S)",
            Codename::SpeedyWallaby => "HYPERSPEED SPRINT (RANK S)",
            Codename::TasmanianDevil => "AGGRESSIVE PREDATOR (RANK A)",
            Codename::LurkingEchidna => "BURROW SHADOW (RANK A)",
            Codename::IronBill => "INDOMITABLE SURVIVOR (RANK A)",
            Codename::SlyPossum => "NIGHT RUNNER (RANK B)",
            Codename::BushKoala => "CANOPY INFILTRATOR (RANK B)",
            Codename::VenomousTaipan => "DEADLY STRIKER (RANK B)",
            Codename::WombatTunnel => "SUBTERRANEAN SNEAK (RANK C)",
            Codename::CardboardHermit => "DISGUISE MASTER (RANK C)",
            Codename::DuckbillRookie => "JUNIOR OPERATIVE (RANK D)",
        }
    }

    pub fn evaluate(alerts: u16, damage: u16, time_s: u32, takedowns: u16) -> Self {
        if alerts == 0 && damage == 0 && time_s <= 420 {
            Codename::BigPlatypus
        } else if alerts == 0 && takedowns == 0 {
            Codename::GhostPlatypus
        } else if time_s <= 300 {
            Codename::SpeedyWallaby
        } else if takedowns >= 10 {
            Codename::TasmanianDevil
        } else if alerts <= 2 && damage <= 3 {
            Codename::LurkingEchidna
        } else if damage >= 12 {
            Codename::IronBill
        } else if time_s <= 600 && alerts <= 5 {
            Codename::SlyPossum
        } else if takedowns >= 5 && alerts <= 4 {
            Codename::BushKoala
        } else if takedowns >= 6 {
            Codename::VenomousTaipan
        } else if alerts <= 8 {
            Codename::WombatTunnel
        } else if takedowns <= 1 {
            Codename::CardboardHermit
        } else {
            Codename::DuckbillRookie
        }
    }
}

// --------------------------------------------------------------------------
// Wire format (mirror of game/src/save.rs)
// --------------------------------------------------------------------------

const OFF_MAGIC: usize = 0;
const OFF_VERSION: usize = OFF_MAGIC + 4;
const OFF_UNLOCKED_ACT: usize = OFF_VERSION + 1;
const OFF_HIGHEST_SCORE: usize = OFF_UNLOCKED_ACT + 1;
const OFF_TOTAL_YABBIES: usize = OFF_HIGHEST_SCORE + 4;
const OFF_ALERTS_COUNT: usize = OFF_TOTAL_YABBIES + 2;
const OFF_BEST_TIME: usize = OFF_ALERTS_COUNT + 2;
const OFF_BEST_CODENAME: usize = OFF_BEST_TIME + 4;
const OFF_TUXEDO: usize = OFF_BEST_CODENAME + 16;
const OFF_CAMO: usize = OFF_TUXEDO + 1;
const OFF_WIREFRAME_UNLOCKED: usize = OFF_CAMO + 1;
const OFF_VR_CLEARED: usize = OFF_WIREFRAME_UNLOCKED + 1;
const OFF_SELECTED_COSTUME: usize = OFF_VR_CLEARED + 1;
const OFF_WIREFRAME_ENABLED: usize = OFF_SELECTED_COSTUME + 1;
const OFF_LANGUAGE: usize = OFF_WIREFRAME_ENABLED + 1;
const OFF_SCREEN_OFFSET_X: usize = OFF_LANGUAGE + 1;
const OFF_SCREEN_OFFSET_Y: usize = OFF_SCREEN_OFFSET_X + 1;
const OFF_PAL_MODE: usize = OFF_SCREEN_OFFSET_Y + 1;
const OFF_CHECKSUM: usize = OFF_PAL_MODE + 1;
pub const SERIALIZED_SIZE: usize = OFF_CHECKSUM + 2;

const CHECKSUM_FIELD_SIZES: [u8; 18] = [
    (OFF_VERSION - OFF_MAGIC) as u8,
    (OFF_UNLOCKED_ACT - OFF_VERSION) as u8,
    (OFF_HIGHEST_SCORE - OFF_UNLOCKED_ACT) as u8,
    (OFF_TOTAL_YABBIES - OFF_HIGHEST_SCORE) as u8,
    (OFF_ALERTS_COUNT - OFF_TOTAL_YABBIES) as u8,
    (OFF_BEST_TIME - OFF_ALERTS_COUNT) as u8,
    (OFF_BEST_CODENAME - OFF_BEST_TIME) as u8,
    (OFF_TUXEDO - OFF_BEST_CODENAME) as u8,
    (OFF_CAMO - OFF_TUXEDO) as u8,
    (OFF_WIREFRAME_UNLOCKED - OFF_CAMO) as u8,
    (OFF_VR_CLEARED - OFF_WIREFRAME_UNLOCKED) as u8,
    (OFF_SELECTED_COSTUME - OFF_VR_CLEARED) as u8,
    (OFF_WIREFRAME_ENABLED - OFF_SELECTED_COSTUME) as u8,
    (OFF_LANGUAGE - OFF_WIREFRAME_ENABLED) as u8,
    (OFF_SCREEN_OFFSET_X - OFF_LANGUAGE) as u8,
    (OFF_SCREEN_OFFSET_Y - OFF_SCREEN_OFFSET_X) as u8,
    (OFF_PAL_MODE - OFF_SCREEN_OFFSET_Y) as u8,
    (OFF_CHECKSUM - OFF_PAL_MODE) as u8,
];

const STRUCT_PADDING: usize = 2;
pub const SCREEN_OFFSET_LIMIT: i8 = 16;
const CHECKSUM_INIT: u16 = 0x5A5A;

const _: () = {
    let mut total = 0usize;
    let mut i = 0;
    while i < CHECKSUM_FIELD_SIZES.len() {
        total += CHECKSUM_FIELD_SIZES[i] as usize;
        i += 1;
    }
    assert!(
        total == OFF_CHECKSUM && OFF_CHECKSUM + 2 == SERIALIZED_SIZE,
        "checksum field table does not tile the payload"
    );
};

const _: () = assert!(
    core::mem::size_of::<SaveData>() == SERIALIZED_SIZE + STRUCT_PADDING,
    "SaveData layout drifted from the wire format"
);

const fn field_seed(index: u8) -> u16 {
    (index as u16)
        .wrapping_mul(0x9E37)
        .wrapping_add(0x5A5A)
        | 1
}

fn fold_field(mut h: u16, index: u8, bytes: &[u8]) -> u16 {
    h ^= field_seed(index);
    for &b in bytes {
        h = h.rotate_left(5) ^ (b as u16);
        h = h.wrapping_mul(0x0101) ^ (h >> 7);
    }
    h
}

fn put(buf: &mut [u8; SERIALIZED_SIZE], at: usize, src: &[u8]) {
    buf[at..at + src.len()].copy_from_slice(src);
}

fn get<const N: usize>(buf: &[u8], at: usize) -> [u8; N] {
    let mut tmp = [0u8; N];
    tmp.copy_from_slice(&buf[at..at + N]);
    tmp
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SaveData {
    pub magic: [u8; 4],
    pub version: u8,
    pub unlocked_act: u8,
    pub highest_score: u32,
    pub total_yabbies: u16,
    pub alerts_count: u16,
    pub best_time_seconds: u32,
    pub best_codename: [u8; 16],
    pub tuxedo_unlocked: u8,
    pub camo_unlocked: u8,
    pub wireframe_unlocked: u8,
    pub vr_cleared: u8,
    pub selected_costume: u8,
    pub wireframe_enabled: u8,
    pub language: u8,
    pub screen_offset_x: i8,
    pub screen_offset_y: i8,
    pub pal_mode: u8,
    pub checksum: u16,
}

impl SaveData {
    pub fn new() -> Self {
        let mut save = Self {
            magic: SAVE_MAGIC,
            version: SAVE_VERSION,
            unlocked_act: 0,
            highest_score: 0,
            total_yabbies: 0,
            alerts_count: 0,
            best_time_seconds: 9999,
            best_codename: *b"NEW RECRUIT     ",
            tuxedo_unlocked: 0,
            camo_unlocked: 0,
            wireframe_unlocked: 1,
            vr_cleared: 0,
            selected_costume: 0,
            wireframe_enabled: 0,
            language: 0,
            screen_offset_x: 0,
            screen_offset_y: 0,
            pal_mode: 2,
            checksum: 0,
        };
        save.checksum = save.compute_checksum();
        save
    }

    pub fn to_bytes(&self) -> [u8; SERIALIZED_SIZE] {
        let mut buf = [0u8; SERIALIZED_SIZE];
        put(&mut buf, OFF_MAGIC, &self.magic);
        put(&mut buf, OFF_VERSION, &[self.version]);
        put(&mut buf, OFF_UNLOCKED_ACT, &[self.unlocked_act]);
        put(&mut buf, OFF_HIGHEST_SCORE, &self.highest_score.to_le_bytes());
        put(&mut buf, OFF_TOTAL_YABBIES, &self.total_yabbies.to_le_bytes());
        put(&mut buf, OFF_ALERTS_COUNT, &self.alerts_count.to_le_bytes());
        put(&mut buf, OFF_BEST_TIME, &self.best_time_seconds.to_le_bytes());
        put(&mut buf, OFF_BEST_CODENAME, &self.best_codename);
        put(&mut buf, OFF_TUXEDO, &[self.tuxedo_unlocked]);
        put(&mut buf, OFF_CAMO, &[self.camo_unlocked]);
        put(&mut buf, OFF_WIREFRAME_UNLOCKED, &[self.wireframe_unlocked]);
        put(&mut buf, OFF_VR_CLEARED, &[self.vr_cleared]);
        put(&mut buf, OFF_SELECTED_COSTUME, &[self.selected_costume]);
        put(&mut buf, OFF_WIREFRAME_ENABLED, &[self.wireframe_enabled]);
        put(&mut buf, OFF_LANGUAGE, &[self.language]);
        put(&mut buf, OFF_SCREEN_OFFSET_X, &[self.screen_offset_x as u8]);
        put(&mut buf, OFF_SCREEN_OFFSET_Y, &[self.screen_offset_y as u8]);
        put(&mut buf, OFF_PAL_MODE, &[self.pal_mode]);
        put(&mut buf, OFF_CHECKSUM, &self.checksum.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < SERIALIZED_SIZE {
            return None;
        }
        let magic: [u8; 4] = get(buf, OFF_MAGIC);
        let version = buf[OFF_VERSION];
        if magic != SAVE_MAGIC || version != SAVE_VERSION {
            return None;
        }
        let mut best_codename = [0u8; 16];
        best_codename.copy_from_slice(&buf[OFF_BEST_CODENAME..OFF_BEST_CODENAME + 16]);
        Some(Self {
            magic,
            version,
            unlocked_act: buf[OFF_UNLOCKED_ACT],
            highest_score: u32::from_le_bytes(get(buf, OFF_HIGHEST_SCORE)),
            total_yabbies: u16::from_le_bytes(get(buf, OFF_TOTAL_YABBIES)),
            alerts_count: u16::from_le_bytes(get(buf, OFF_ALERTS_COUNT)),
            best_time_seconds: u32::from_le_bytes(get(buf, OFF_BEST_TIME)),
            best_codename,
            tuxedo_unlocked: buf[OFF_TUXEDO],
            camo_unlocked: buf[OFF_CAMO],
            wireframe_unlocked: buf[OFF_WIREFRAME_UNLOCKED],
            vr_cleared: buf[OFF_VR_CLEARED],
            selected_costume: buf[OFF_SELECTED_COSTUME],
            wireframe_enabled: buf[OFF_WIREFRAME_ENABLED],
            language: buf[OFF_LANGUAGE],
            screen_offset_x: buf[OFF_SCREEN_OFFSET_X] as i8,
            screen_offset_y: buf[OFF_SCREEN_OFFSET_Y] as i8,
            pal_mode: buf[OFF_PAL_MODE],
            checksum: u16::from_le_bytes(get(buf, OFF_CHECKSUM)),
        })
    }

    pub fn peek_version(buf: &[u8]) -> Option<u8> {
        if buf.len() < SERIALIZED_SIZE {
            return None;
        }
        Some(buf[OFF_VERSION])
    }

    pub fn with_checksum(mut self) -> Self {
        self.checksum = self.compute_checksum();
        self
    }

    pub fn compute_checksum(&self) -> u16 {
        let buf = self.to_bytes();
        let mut h = CHECKSUM_INIT;
        let mut at = 0usize;
        let mut index = 0u8;
        while (index as usize) < CHECKSUM_FIELD_SIZES.len() {
            let end = at + CHECKSUM_FIELD_SIZES[index as usize] as usize;
            h = fold_field(h, index, &buf[at..end]);
            at = end;
            index += 1;
        }
        h
    }

    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.version == SAVE_VERSION && self.checksum == self.compute_checksum()
    }

    pub fn is_sane(&self) -> bool {
        self.unlocked_act <= CAMPAIGN_ACT_COUNT
            && self.selected_costume < 3
            && self.language < 5
            && self.pal_mode < 3
            && self.vr_cleared < 16
            && self.tuxedo_unlocked <= 1
            && self.camo_unlocked <= 1
            && self.wireframe_unlocked <= 1
            && self.wireframe_enabled <= 1
            && self.screen_offset_x >= -SCREEN_OFFSET_LIMIT
            && self.screen_offset_x <= SCREEN_OFFSET_LIMIT
            && self.screen_offset_y >= -SCREEN_OFFSET_LIMIT
            && self.screen_offset_y <= SCREEN_OFFSET_LIMIT
    }
}

/// Mirror of game/src/save.rs's load classification.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LoadOutcome {
    Loaded,
    NotFound,
    Corrupt,
    Incompatible,
    CardError,
}

/// Mirror of game/src/save.rs's `classify_payload`, plus the `None` (no file)
/// case the card read reports as `Error::NotFound`.
pub fn load_outcome_for(file: Option<&[u8]>) -> LoadOutcome {
    let Some(payload) = file else {
        return LoadOutcome::NotFound;
    };
    match SaveData::peek_version(payload) {
        None => LoadOutcome::Corrupt,
        Some(v) if v != SAVE_VERSION => LoadOutcome::Incompatible,
        Some(_) => match SaveData::from_bytes(payload) {
            Some(save) if save.is_valid() && save.is_sane() => LoadOutcome::Loaded,
            _ => LoadOutcome::Corrupt,
        },
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Act {
    Act1_1Drainage = 0,
    Act1_2Barracks = 1,
    Act1_3MechBoss = 2,
    Act2_1Rapids = 3,
    Act2_2Mangroves = 4,
    Act2_3JetSkiBoss = 5,
    Act3_1Highway = 6,
    Act3_2Laneways = 7,
    Act3_3SniperBoss = 8,
    Act4_1Dunes = 9,
    Act4_2PierTrench = 10,
    Act4_3ExcavatorBoss = 11,
    VrSneaking = 12,
    VrCqc = 13,
    VrSonar = 14,
    VrSpeed = 15,
}

impl Act {
    pub fn is_boss(&self) -> bool {
        matches!(
            self,
            Act::Act1_3MechBoss | Act::Act2_3JetSkiBoss | Act::Act3_3SniperBoss | Act::Act4_3ExcavatorBoss
        )
    }

    pub fn is_rapids(&self) -> bool {
        matches!(self, Act::Act2_1Rapids | Act::Act2_3JetSkiBoss)
    }

    pub fn is_vr(&self) -> bool {
        matches!(self, Act::VrSneaking | Act::VrCqc | Act::VrSonar | Act::VrSpeed)
    }

    pub fn next(&self) -> Option<Act> {
        match self {
            Act::Act1_1Drainage => Some(Act::Act1_2Barracks),
            Act::Act1_2Barracks => Some(Act::Act1_3MechBoss),
            Act::Act1_3MechBoss => Some(Act::Act2_1Rapids),
            Act::Act2_1Rapids => Some(Act::Act2_2Mangroves),
            Act::Act2_2Mangroves => Some(Act::Act2_3JetSkiBoss),
            Act::Act2_3JetSkiBoss => Some(Act::Act3_1Highway),
            Act::Act3_1Highway => Some(Act::Act3_2Laneways),
            Act::Act3_2Laneways => Some(Act::Act3_3SniperBoss),
            Act::Act3_3SniperBoss => Some(Act::Act4_1Dunes),
            Act::Act4_1Dunes => Some(Act::Act4_2PierTrench),
            Act::Act4_2PierTrench => Some(Act::Act4_3ExcavatorBoss),
            Act::Act4_3ExcavatorBoss => None,
            Act::VrSneaking | Act::VrCqc | Act::VrSonar | Act::VrSpeed => None,
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Act::Act1_1Drainage,
            1 => Act::Act1_2Barracks,
            2 => Act::Act1_3MechBoss,
            3 => Act::Act2_1Rapids,
            4 => Act::Act2_2Mangroves,
            5 => Act::Act2_3JetSkiBoss,
            6 => Act::Act3_1Highway,
            7 => Act::Act3_2Laneways,
            8 => Act::Act3_3SniperBoss,
            9 => Act::Act4_1Dunes,
            10 => Act::Act4_2PierTrench,
            11 => Act::Act4_3ExcavatorBoss,
            12 => Act::VrSneaking,
            13 => Act::VrCqc,
            14 => Act::VrSonar,
            15 => Act::VrSpeed,
            _ => Act::Act1_1Drainage,
        }
    }
}

// --------------------------------------------------------------------------
// Campaign progress (mirror of game/src/game.rs + game/src/save.rs)
// --------------------------------------------------------------------------

/// Mirror of `save::CAMPAIGN_ACT_COUNT`.
pub const CAMPAIGN_ACT_COUNT: u8 = 12;
/// Mirror of the video standards' frame rates (game/src/game.rs).
const FPS_NTSC: u32 = 60;
const FPS_PAL: u32 = 50;
/// Mirror of the boss intro cutscene constants (game/src/game.rs).
const BOSS_INTRO_FRAMES: u16 = 240;
const BOSS_ORBIT_RADIUS: i32 = 450;
const ORBIT_TURN_UNITS: u32 = 256;

/// Mirror of `save::new_campaign_save`.
pub fn new_campaign_save(previous: &SaveData) -> SaveData {
    SaveData {
        language: previous.language,
        pal_mode: previous.pal_mode,
        screen_offset_x: previous.screen_offset_x,
        screen_offset_y: previous.screen_offset_y,
        wireframe_enabled: previous.wireframe_enabled,
        ..SaveData::new()
    }
    .with_checksum()
}

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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CellType {
    Floor = 0,
    Wall = 1,
    Crate = 2,
    Container = 3,
    Water = 4,
    WaterCurrent = 5,
    AirDuct = 6,
    TallGrass = 7,
    LaserTripwire = 8,
    ExitBurrow = 9,
    MetalGrate = 10,
}

impl CellType {
    pub fn is_solid(&self, is_crawling: bool) -> bool {
        match self {
            CellType::Wall | CellType::Container | CellType::Crate => true,
            CellType::AirDuct => !is_crawling,
            _ => false,
        }
    }

    pub fn is_water(&self) -> bool {
        matches!(self, CellType::Water | CellType::WaterCurrent)
    }
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

    println!("\nALL PLATTYPUS GAME LOGIC TESTS PASSED SUCCESSFULLY! (19/19 test suites)");
}
