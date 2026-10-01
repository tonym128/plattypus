// SPDX-License-Identifier: GPL-2.0-or-later
//! Shared save data types, serialization wire format, and codename ranking
//! for Plattypus. Pure no_std with zero hardware dependencies.

pub const SAVE_FILENAME: &str = "BASLUS-00001PLATTY";
pub const SAVE_TITLE: &str = "PLATTYPUS MGS";

pub const SAVE_MAGIC: [u8; 4] = *b"PLTY";
/// Bumped from 3: the checksum is no longer an additive sum, so a v3 card's
/// stored sum cannot be verified by a v4 build. Those cards are reported as an
/// incompatible save (never as a missing one) instead of failing as corruption.
pub const SAVE_VERSION: u8 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Language {
    English = 0,
    French = 1,
    German = 2,
    Spanish = 3,
    Japanese = 4,
}

impl Language {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Language::French,
            2 => Language::German,
            3 => Language::Spanish,
            4 => Language::Japanese,
            _ => Language::English,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "ENGLISH",
            Language::French => "FRANCAIS",
            Language::German => "DEUTSCH",
            Language::Spanish => "ESPANOL",
            Language::Japanese => "NIHONGO (ROMAJI)",
        }
    }
}

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
// Wire format
// --------------------------------------------------------------------------

pub const OFF_MAGIC: usize = 0;
pub const OFF_VERSION: usize = OFF_MAGIC + 4;
pub const OFF_UNLOCKED_ACT: usize = OFF_VERSION + 1;
pub const OFF_HIGHEST_SCORE: usize = OFF_UNLOCKED_ACT + 1;
pub const OFF_TOTAL_YABBIES: usize = OFF_HIGHEST_SCORE + 4;
pub const OFF_ALERTS_COUNT: usize = OFF_TOTAL_YABBIES + 2;
pub const OFF_BEST_TIME: usize = OFF_ALERTS_COUNT + 2;
pub const OFF_BEST_CODENAME: usize = OFF_BEST_TIME + 4;
pub const OFF_TUXEDO: usize = OFF_BEST_CODENAME + 16;
pub const OFF_CAMO: usize = OFF_TUXEDO + 1;
pub const OFF_WIREFRAME_UNLOCKED: usize = OFF_CAMO + 1;
pub const OFF_VR_CLEARED: usize = OFF_WIREFRAME_UNLOCKED + 1;
pub const OFF_SELECTED_COSTUME: usize = OFF_VR_CLEARED + 1;
pub const OFF_WIREFRAME_ENABLED: usize = OFF_SELECTED_COSTUME + 1;
pub const OFF_LANGUAGE: usize = OFF_WIREFRAME_ENABLED + 1;
pub const OFF_SCREEN_OFFSET_X: usize = OFF_LANGUAGE + 1;
pub const OFF_SCREEN_OFFSET_Y: usize = OFF_SCREEN_OFFSET_X + 1;
pub const OFF_PAL_MODE: usize = OFF_SCREEN_OFFSET_Y + 1;
pub const OFF_CHECKSUM: usize = OFF_PAL_MODE + 1;
pub const SERIALIZED_SIZE: usize = OFF_CHECKSUM + 2;

pub const CHECKSUM_FIELD_SIZES: [u8; 18] = [
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

pub const STRUCT_PADDING: usize = 2;
pub const SCREEN_OFFSET_LIMIT: i8 = 16;
pub const CAMPAIGN_ACT_COUNT: u8 = 12;
pub const CHECKSUM_INIT: u16 = 0x5A5A;

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

pub const fn field_seed(index: u8) -> u16 {
    (index as u16)
        .wrapping_mul(0x9E37)
        .wrapping_add(0x5A5A)
        | 1
}

pub fn fold_field(mut h: u16, index: u8, bytes: &[u8]) -> u16 {
    h ^= field_seed(index);
    for &b in bytes {
        h = h.rotate_left(5) ^ (b as u16);
        h = h.wrapping_mul(0x0101) ^ (h >> 7);
    }
    h
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(C)]
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

const _: () = assert!(
    core::mem::size_of::<SaveData>() == SERIALIZED_SIZE + STRUCT_PADDING,
    "SaveData layout drifted from the wire format: update the offsets, to_bytes and from_bytes"
);

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

impl Default for SaveData {
    fn default() -> Self {
        Self::new()
    }
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
            wireframe_unlocked: 1, // Available by default for retro PS1 fans!
            vr_cleared: 0,
            selected_costume: 0,
            wireframe_enabled: 0,
            language: 0,
            screen_offset_x: 0,
            screen_offset_y: 0,
            pal_mode: 2, // Auto-detect
            checksum: 0,
        };
        save.checksum = save.compute_checksum();
        save
    }

    pub fn reset_to_clean_slate(&mut self) {
        *self = Self::new();
    }

    pub fn with_checksum(mut self) -> Self {
        self.checksum = self.compute_checksum();
        self
    }

    pub fn compute_checksum(&self) -> u16 {
        let mut h = CHECKSUM_INIT;
        h = fold_field(h, 0, &self.magic);
        h = fold_field(h, 1, &[self.version]);
        h = fold_field(h, 2, &[self.unlocked_act]);
        h = fold_field(h, 3, &self.highest_score.to_le_bytes());
        h = fold_field(h, 4, &self.total_yabbies.to_le_bytes());
        h = fold_field(h, 5, &self.alerts_count.to_le_bytes());
        h = fold_field(h, 6, &self.best_time_seconds.to_le_bytes());
        h = fold_field(h, 7, &self.best_codename);
        h = fold_field(h, 8, &[self.tuxedo_unlocked]);
        h = fold_field(h, 9, &[self.camo_unlocked]);
        h = fold_field(h, 10, &[self.wireframe_unlocked]);
        h = fold_field(h, 11, &[self.vr_cleared]);
        h = fold_field(h, 12, &[self.selected_costume]);
        h = fold_field(h, 13, &[self.wireframe_enabled]);
        h = fold_field(h, 14, &[self.language]);
        h = fold_field(h, 15, &[self.screen_offset_x as u8]);
        h = fold_field(h, 16, &[self.screen_offset_y as u8]);
        h = fold_field(h, 17, &[self.pal_mode]);
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


    pub fn to_bytes(&self) -> [u8; SERIALIZED_SIZE] {
        let mut buf = [0u8; SERIALIZED_SIZE];
        buf[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&self.magic);
        buf[OFF_VERSION] = self.version;
        buf[OFF_UNLOCKED_ACT] = self.unlocked_act;
        buf[OFF_HIGHEST_SCORE..OFF_HIGHEST_SCORE + 4].copy_from_slice(&self.highest_score.to_le_bytes());
        buf[OFF_TOTAL_YABBIES..OFF_TOTAL_YABBIES + 2].copy_from_slice(&self.total_yabbies.to_le_bytes());
        buf[OFF_ALERTS_COUNT..OFF_ALERTS_COUNT + 2].copy_from_slice(&self.alerts_count.to_le_bytes());
        buf[OFF_BEST_TIME..OFF_BEST_TIME + 4].copy_from_slice(&self.best_time_seconds.to_le_bytes());
        buf[OFF_BEST_CODENAME..OFF_BEST_CODENAME + 16].copy_from_slice(&self.best_codename);
        buf[OFF_TUXEDO] = self.tuxedo_unlocked;
        buf[OFF_CAMO] = self.camo_unlocked;
        buf[OFF_WIREFRAME_UNLOCKED] = self.wireframe_unlocked;
        buf[OFF_VR_CLEARED] = self.vr_cleared;
        buf[OFF_SELECTED_COSTUME] = self.selected_costume;
        buf[OFF_WIREFRAME_ENABLED] = self.wireframe_enabled;
        buf[OFF_LANGUAGE] = self.language;
        buf[OFF_SCREEN_OFFSET_X] = self.screen_offset_x as u8;
        buf[OFF_SCREEN_OFFSET_Y] = self.screen_offset_y as u8;
        buf[OFF_PAL_MODE] = self.pal_mode;
        buf[OFF_CHECKSUM..OFF_CHECKSUM + 2].copy_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    pub fn from_bytes(payload: &[u8]) -> Option<Self> {
        if payload.len() < SERIALIZED_SIZE {
            return None;
        }
        let magic: [u8; 4] = payload[OFF_MAGIC..OFF_MAGIC + 4].try_into().ok()?;
        let highest_score = u32::from_le_bytes(payload[OFF_HIGHEST_SCORE..OFF_HIGHEST_SCORE + 4].try_into().ok()?);
        let total_yabbies = u16::from_le_bytes(payload[OFF_TOTAL_YABBIES..OFF_TOTAL_YABBIES + 2].try_into().ok()?);
        let alerts_count = u16::from_le_bytes(payload[OFF_ALERTS_COUNT..OFF_ALERTS_COUNT + 2].try_into().ok()?);
        let best_time_seconds = u32::from_le_bytes(payload[OFF_BEST_TIME..OFF_BEST_TIME + 4].try_into().ok()?);
        let mut best_codename = [0u8; 16];
        best_codename.copy_from_slice(&payload[OFF_BEST_CODENAME..OFF_BEST_CODENAME + 16]);
        let checksum = u16::from_le_bytes(payload[OFF_CHECKSUM..OFF_CHECKSUM + 2].try_into().ok()?);

        Some(Self {
            magic,
            version: payload[OFF_VERSION],
            unlocked_act: payload[OFF_UNLOCKED_ACT],
            highest_score,
            total_yabbies,
            alerts_count,
            best_time_seconds,
            best_codename,
            tuxedo_unlocked: payload[OFF_TUXEDO],
            camo_unlocked: payload[OFF_CAMO],
            wireframe_unlocked: payload[OFF_WIREFRAME_UNLOCKED],
            vr_cleared: payload[OFF_VR_CLEARED],
            selected_costume: payload[OFF_SELECTED_COSTUME],
            wireframe_enabled: payload[OFF_WIREFRAME_ENABLED],
            language: payload[OFF_LANGUAGE],
            screen_offset_x: payload[OFF_SCREEN_OFFSET_X] as i8,
            screen_offset_y: payload[OFF_SCREEN_OFFSET_Y] as i8,
            pal_mode: payload[OFF_PAL_MODE],
            checksum,
        })
    }

    pub fn peek_version(payload: &[u8]) -> Option<u8> {
        if payload.len() < SERIALIZED_SIZE || payload[OFF_MAGIC..OFF_MAGIC + 4] != SAVE_MAGIC {
            None
        } else {
            Some(payload[OFF_VERSION])
        }
    }
}

pub fn service_records_from(save: &SaveData) -> ([u8; 20], [u8; 20], [u8; 20]) {
    let mut score_buf = [b' '; 20];
    let mut time_buf = [b' '; 20];
    let mut rank_buf = [b' '; 20];
    write_u32_padded(&mut score_buf, b"BEST SCORE: ", save.highest_score);
    if save.best_time_seconds > 0 {
        write_u32_padded(&mut time_buf, b"BEST TIME: ", save.best_time_seconds);
    } else {
        copy_ascii(&mut time_buf, b"BEST TIME: --");
    }
    let mut rank_ok = false;
    for (i, b) in save.best_codename.iter().enumerate() {
        rank_buf[i] = *b;
        if *b != b' ' {
            rank_ok = true;
        }
    }
    if !rank_ok {
        copy_ascii(&mut rank_buf, b"RANK: --");
    } else {
        rank_buf[0] = b'R';
        rank_buf[1] = b'A';
        rank_buf[2] = b'N';
        rank_buf[3] = b'K';
        rank_buf[4] = b':';
        rank_buf[5] = b' ';
    }
    (score_buf, time_buf, rank_buf)
}

fn copy_ascii(dst: &mut [u8; 20], src: &[u8]) {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
}

fn write_u32_padded(dst: &mut [u8; 20], label: &[u8], value: u32) {
    copy_ascii(dst, label);
    let mut digits = [0u8; 10];
    let mut v = value;
    let mut n = 0;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 || n == 10 {
            break;
        }
    }
    let start = label.len();
    for i in 0..n {
        if start + i < dst.len() {
            dst[start + i] = digits[n - 1 - i];
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LoadOutcome {
    Loaded,
    NotFound,
    Corrupt,
    Incompatible,
    CardError,
}

impl LoadOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            LoadOutcome::Loaded => "CARD: SAVE OK",
            LoadOutcome::NotFound => "CARD: NO SAVE",
            LoadOutcome::Incompatible => "CARD: TOO OLD",
            LoadOutcome::Corrupt => "CARD: CORRUPT",
            LoadOutcome::CardError => "CARD: ERROR",
        }
    }
}

pub fn classify_payload(payload: &[u8]) -> LoadOutcome {
    match SaveData::peek_version(payload) {
        None => LoadOutcome::Corrupt,
        Some(v) if v != SAVE_VERSION => LoadOutcome::Incompatible,
        Some(_) => match SaveData::from_bytes(payload) {
            Some(save) if save.is_valid() && save.is_sane() => LoadOutcome::Loaded,
            _ => LoadOutcome::Corrupt,
        },
    }
}

pub fn load_outcome_for(file: Option<&[u8]>) -> LoadOutcome {
    let Some(payload) = file else {
        return LoadOutcome::NotFound;
    };
    classify_payload(payload)
}

