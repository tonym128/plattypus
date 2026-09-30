//! PlayStation 1 Memory Card manager, save data serialization,
//! and animated 16x16 BIOS save icon for Plattypus MGS.

use psx_mc::{Card, HardwareCard, SaveIcon, Slot};

pub const SAVE_FILENAME: &str = "BASLUS-00001PLATTY";
pub const SAVE_TITLE: &str = "PLATTYPUS MGS";

const SAVE_MAGIC: [u8; 4] = *b"PLTY";
/// Bumped from 3: the checksum is no longer an additive sum, so a v3 card's
/// stored sum cannot be verified by a v4 build. Those cards are reported as an
/// incompatible save (never as a missing one) instead of failing as corruption.
const SAVE_VERSION: u8 = 4;

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

/// Address of the region character inside the BIOS version/date string.
const BIOS_REGION_ADDR: usize = 0xBFC7FF52;

/// Read the console's region character out of the BIOS version string.
///
/// `None` when the byte is not one of the known region characters, which is
/// what an unmapped or differently-laid-out BIOS revision reads as. Callers
/// must handle that instead of falling through a catch-all match arm and
/// claiming a region the hardware never reported.
pub fn detect_region_char() -> Option<u8> {
    // SAFETY: the BIOS is memory-mapped on this hardware and the address is
    // fixed for the console family. A revision without the string still reads
    // a byte, which `is_region_char` rejects, so there is no panic path.
    let raw = unsafe { core::ptr::read_volatile(BIOS_REGION_ADDR as *const u8) };
    is_region_char(raw).then_some(raw)
}

fn is_region_char(c: u8) -> bool {
    matches!(
        c,
        b'U' | b'E' | b'J' | b'P' | b'D' | b'I' | b'K' | b'C' | b'A' | b'S' | b'B' | b'N'
    )
}

/// Auto-detect PlayStation hardware video standard (PAL 50Hz vs NTSC 60Hz)
/// by inspecting the system ROM date string in the BIOS.
pub fn detect_console_region() -> (psx_gpu::VideoMode, &'static str) {
    match detect_region_char() {
        Some(b'E') | Some(b'P') | Some(b'S') => (psx_gpu::VideoMode::Pal, "PAL 50HZ"),
        Some(b'J') | Some(b'I') | Some(b'K') => (psx_gpu::VideoMode::Ntsc, "NTSC-J 60HZ"),
        Some(_) => (psx_gpu::VideoMode::Ntsc, "NTSC-U 60HZ"),
        // Unrecognised byte: NTSC is the conservative default, but say so
        // rather than reporting a region the BIOS never claimed.
        None => (psx_gpu::VideoMode::Ntsc, "BIOS UNKNOWN"),
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
//
// `SaveData` is `#[repr(C)]`, not packed, so it carries an alignment hole after
// `unlocked_act` (see `STRUCT_PADDING` below). The struct is therefore *not* the
// wire format and is never memcpy'd: the payload is assembled and parsed one
// field at a time, which also makes the on-card bytes a pure function of the
// values (the old struct-blob copy persisted 2 uninitialized stack bytes).

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
/// Bytes in the on-card payload.
/// Three short "LABEL VALUE" strings summarising the service record for the
/// stage-select panel: best score, best mission time, and best rank.
///
/// These fields have been written on every clear since the save format was
/// introduced but were never read by any screen, so the player had no number to
/// beat and no history to compare against. Formatting lives here rather than in
/// the renderer so the buffer bounds are checked in one place.
///
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
        // Prefix over the first five bytes.
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

pub const SERIALIZED_SIZE: usize = OFF_CHECKSUM + 2;

/// Byte length of each checksummed field, derived from the offsets above so the
/// two cannot drift. The trailing `checksum` field is absent: it is the value
/// being computed.
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

/// The `highest_score: u32` alignment hole between `unlocked_act` and
/// `highest_score` in the struct. The wire format does not have it.
const STRUCT_PADDING: usize = 2;

/// Screen-offset clamp the options menu enforces on `screen_offset_x/y`.
pub const SCREEN_OFFSET_LIMIT: i8 = 16;

/// Number of acts in the campaign (Act 1-1 .. Act 4-3).
///
/// `unlocked_act` is a campaign *clear count* in `0..=CAMPAIGN_ACT_COUNT`,
/// not an `Act` index: `0..=10` is the next act to play, `11` is Act 4-3, and
/// `CAMPAIGN_ACT_COUNT` means the campaign is finished. It used to be an
/// `Act` index that doubled as the completion flag, which made 4-3
/// unreachable from the title and let the CODEC scribe write a VR act's index
/// (12..=15) into it. Nothing writes a VR index any more -- a VR clear records
/// `vr_cleared` -- so a card carrying 13..=15 here is a save written by that
/// bug and is reported as corrupt rather than loaded as "campaign finished".
pub const CAMPAIGN_ACT_COUNT: u8 = 12;

/// The checksummed fields must tile the payload exactly: contiguous, in
/// serialization order, with nothing left over before the `checksum` field.
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

/// A field added, removed or retyped in `SaveData` without updating the wire
/// format above breaks the struct/wire agreement (the struct is padded, the
/// payload is not) and must fail to compile.
const _: () = assert!(
    core::mem::size_of::<SaveData>() == SERIALIZED_SIZE + STRUCT_PADDING,
    "SaveData layout drifted from the wire format: update the offsets, to_bytes and from_bytes"
);

/// Initial checksum state.
const CHECKSUM_INIT: u16 = 0x5A5A;

/// Per-field seed derived from the field's position, so field order is part of
/// the checksum even though the fold is already sequential. Forced odd so the
/// `0x0101` multiply below stays a bijection.
const fn field_seed(index: u8) -> u16 {
    (index as u16)
        .wrapping_mul(0x9E37)
        .wrapping_add(0x5A5A)
        | 1
}

/// Fold one field's bytes into the running checksum.
///
/// An additive sum was rejected: addition is commutative, so *any* permutation
/// of the summed fields produced an identical checksum -- swapping
/// `total_yabbies` with `alerts_count` was undetectable, and so was any pair of
/// fields whose contributions cancelled. Each step here is a bijection on `h`
/// (rotate, xor the byte, multiply by the odd 0x0101, xor the shift), so
/// flipping any single bit of any byte of any field changes the result, and
/// folding fields through `field_seed` makes their order significant.
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
    pub pal_mode: u8, // 0: NTSC 60Hz, 1: PAL 50Hz, 2: Auto Detect
    pub checksum: u16,
}

/// The save "NEW CAMPAIGN" starts from.
///
/// Progression is dropped -- campaign position, rank records, costume unlocks
/// and the VR trainer's clears -- because a new campaign is a new playthrough
/// and the old one was only reachable by never clearing anything (every writer
/// used `max`, so nothing could lower `unlocked_act`). What survives is the
/// player's display setup: language, video standard and CRT offset are
/// settings of the machine, not achievements, and re-asking for them after
/// every restart is a bug report waiting to happen. `wireframe_enabled` also
/// survives because the permission that gates it is granted by default in
/// [`SaveData::new`], so preserving the toggle cannot select something the
/// player has not earned; `selected_costume` does not, because tuxedo and camo
/// are exactly the unlocks being cleared.
pub fn new_campaign_save(previous: &SaveData) -> SaveData {
    SaveData {
        language: previous.language,
        pal_mode: previous.pal_mode,
        screen_offset_x: previous.screen_offset_x,
        screen_offset_y: previous.screen_offset_y,
        wireframe_enabled: previous.wireframe_enabled,
        ..SaveData::new()
    }
    // The struct update inherits the base's checksum, which was computed for
    // the base's field values, not these.
    .with_checksum()
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

    /// Serialize to the exact on-card payload: every field written explicitly
    /// and little-endian, no padding, so identical values always produce
    /// identical bytes and no uninitialized memory can reach the card.
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

    /// Parse a payload produced by [`SaveData::to_bytes`].
    ///
    /// The version is checked before any later field is interpreted: a payload
    /// from another save version has a different field layout once the padding
    /// hole is accounted for, and must be rejected rather than misread.
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

    /// The save version byte of a stored payload, without parsing the rest, so
    /// a caller can tell "save from another build" from "corrupt save".
    pub fn peek_version(buf: &[u8]) -> Option<u8> {
        if buf.len() < SERIALIZED_SIZE {
            return None;
        }
        Some(buf[OFF_VERSION])
    }

    /// A copy with a freshly computed checksum. Callers mutate `SaveData` in
    /// place and hand it straight to a save, so the stored checksum has to be
    /// refreshed at the write, not only at construction.
    pub fn with_checksum(mut self) -> Self {
        self.checksum = self.compute_checksum();
        self
    }

    /// Position-dependent checksum over every byte of every field except the
    /// checksum itself. Folding the serialized payload (rather than summing
    /// struct fields) means the checksum covers exactly the bytes on the card,
    /// high halves of 32-bit fields included.
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

    /// Structural check: the magic, the version, and a checksum that matches
    /// the bytes.
    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.version == SAVE_VERSION && self.checksum == self.compute_checksum()
    }

    /// Field-range check for everything the game consumes as an enum index or a
    /// display offset. `is_valid` only proves the bytes are the ones we wrote;
    /// a self-consistent save can still carry `selected_costume = 200`, which
    /// then lands in the 3-way costume cycle and the 5-way language match.
    ///
    /// Plain counters (`highest_score`, `total_yabbies`, `alerts_count`,
    /// `best_time_seconds`) are deliberately not bounded: every site that shows
    /// them saturates its own output.
    pub fn is_sane(&self) -> bool {
        // A campaign clear count, not an `Act` index: 12 is "all 12 acts
        // cleared" and the VR indices 12..=15 are not reachable any more (see
        // `CAMPAIGN_ACT_COUNT`), so the bound is the count itself.
        self.unlocked_act <= CAMPAIGN_ACT_COUNT
            && self.selected_costume < 3
            && self.language < 5
            && self.pal_mode < 3
            // Four VR sims, one bit each.
            && self.vr_cleared < 16
            && self.tuxedo_unlocked <= 1
            && self.camo_unlocked <= 1
            && self.wireframe_unlocked <= 1
            && self.wireframe_enabled <= 1
            // Written as a range test, not `abs()`: `i8::abs` overflows on
            // -128, which is exactly the value this check has to reject.
            && self.screen_offset_x >= -SCREEN_OFFSET_LIMIT
            && self.screen_offset_x <= SCREEN_OFFSET_LIMIT
            && self.screen_offset_y >= -SCREEN_OFFSET_LIMIT
            && self.screen_offset_y <= SCREEN_OFFSET_LIMIT
    }
}

/// 16-color BGR555 Palette for Platty BIOS icon
/// Format: 0b_0_BBBBB_GGGGG_RRRRR
const ICON_PALETTE: [u16; 16] = [
    0x0000, // 0: Transparent
    0x08A5, // 1: Dark Brown Fur (R:5, G:5, B:2)
    0x14E9, // 2: Medium Brown Fur (R:9, G:7, B:5)
    0x1D2D, // 3: Warm Light Brown Fur (R:13, G:9, B:7)
    0x2571, // 4: Fur Highlight
    0x18C3, // 5: Bill/Tail Dark Charcoal (R:3, G:3, B:6)
    0x2526, // 6: Bill Slate Gray (R:6, G:6, B:9)
    0x0640, // 7: Deep Olive Green (Headband shadow)
    0x0AC2, // 8: Combat Olive-Drab Green (Headband)
    0x1F24, // 9: Bright Bandana Green (R:4, G:25, B:7)
    0x03FF, // 10: Golden Star Yabby Gold (R:31, G:31, B:0)
    0x02DF, // 11: Warm Amber Gold
    0x015A, // 12: Yabby Claw Orange (R:26, G:10, B:0)
    0x7FFF, // 13: Pure White Eye shine / Sparkle
    0x5294, // 14: Soft Light Cyan (Water shine)
    0x0000, // 15: Black eye pupil
];

/// 16x16 pixel 4bpp indexed image (128 bytes total).
/// Depicts Platty's face with his green combat headband and the golden star yabby!
const ICON_PIXELS: [u8; 128] = [
    // Rows 0-1: Empty sky & top of green combat bandana
    0x00, 0x00, 0x88, 0x88, 0x88, 0x88, 0x00, 0x00,
    0x00, 0x88, 0x99, 0x99, 0x99, 0x99, 0x88, 0x00,
    // Rows 2-3: Bandana knots & forehead fur
    0x88, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x88,
    0x00, 0x22, 0x33, 0x33, 0x33, 0x33, 0x22, 0x00,
    // Rows 4-5: Eyes with white shine & brown cheeks
    0x02, 0x33, 0xF0, 0x33, 0x33, 0x0F, 0x33, 0x20,
    0x22, 0x33, 0xDD, 0x33, 0x33, 0xDD, 0x33, 0x22,
    // Rows 6-7: Broad duck bill starting
    0x02, 0x26, 0x66, 0x66, 0x66, 0x66, 0x22, 0x00,
    0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x00,
    // Rows 8-9: Bill nostrils & lower bill
    0x00, 0x66, 0x55, 0x66, 0x66, 0x55, 0x66, 0x00,
    0x00, 0x56, 0x66, 0x66, 0x66, 0x66, 0x65, 0x00,
    // Rows 10-11: Clutched Golden Star Yabby!
    0x00, 0x00, 0xAA, 0xAA, 0xAA, 0xAA, 0x00, 0x00,
    0x00, 0x0A, 0xBB, 0xDD, 0xDD, 0xBB, 0xA0, 0x00,
    // Rows 12-13: Yabby claws and webbed paws
    0x00, 0xAA, 0xBB, 0xDD, 0xDD, 0xBB, 0xAA, 0x00,
    0x02, 0x2A, 0xAA, 0xBB, 0xBB, 0xAA, 0xA2, 0x20,
    // Rows 14-15: Water ripples below
    0xEE, 0x02, 0x22, 0x00, 0x00, 0x22, 0x20, 0xEE,
    0x0E, 0xEE, 0x00, 0xEE, 0xEE, 0x00, 0xEE, 0xE0,
];

pub fn get_platty_save_icon() -> SaveIcon {
    SaveIcon::new(ICON_PALETTE, ICON_PIXELS)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Idle,
    Saving,
    SaveSuccess,
    SaveErrorNoCard,
    SaveErrorUnformatted,
    /// A card answered but its directory could not be read or validated.
    SaveErrorCorrupt,
    SaveErrorFailed,
    Loading,
    LoadSuccess,
    LoadNotFound,
    /// A file was present and could not be read: bad magic, failed checksum, or
    /// a field outside its range.
    LoadError,
    /// A file from a different save version. Kept on the card, never reported
    /// as a missing save.
    LoadIncompatible,
}

/// Result of looking for this game's save on a memory card. Sticky (unlike
/// `SaveStatus`, which clears itself after its timer) so menus can tell the
/// player that a save file exists but was not loaded.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadOutcome {
    Loaded,
    /// No file with our name on either slot. The only case that may be reported
    /// to the player as "no save".
    NotFound,
    /// File present, but the magic, checksum or a field range rejected it.
    Corrupt,
    /// File present, written by a different save version.
    Incompatible,
    /// A card answered but a frame or the transport failed.
    CardError,
}

impl LoadOutcome {
    /// Short status line for the options screen's card banner.
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

/// Classify a payload that was read from under our own file name.
///
/// Absence is not decided here: the caller only reaches this with a file that
/// exists, so a file this build cannot use is never reported as a missing one.
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

pub struct MemoryCardManager {
    pub status: SaveStatus,
    pub status_timer: u16,
    /// Sticky state of this game's save file on the card, updated by both a load
    /// and a successful write, for menus and diagnostics.
    pub last_load: LoadOutcome,
}

/// Read buffer for a save payload. One card frame is 128 bytes, which also
/// covers the 48-byte payload written by the previous save version so those
/// cards can be identified rather than mis-sized.
const SAVE_BUFFER_SIZE: usize = 128;

impl MemoryCardManager {
    pub fn new() -> Self {
        Self {
            status: SaveStatus::Idle,
            status_timer: 0,
            last_load: LoadOutcome::NotFound,
        }
    }

    pub fn update(&mut self) {
        if self.status_timer > 0 {
            self.status_timer -= 1;
            if self.status_timer == 0 {
                self.status = SaveStatus::Idle;
            }
        }
    }

    /// Attempt to save game progress (queries Slot 1 first, falls back to Slot 2).
    pub fn save_to_slot1(&mut self, data: &SaveData) -> bool {
        self.save_game(data)
    }

    /// Write the save to the first slot that is formatted *and* passes the
    /// driver's strict directory validation.
    ///
    /// The already-probed `Card` is carried through to the write instead of
    /// being rebuilt, and a card that answers but cannot be read is reported
    /// apart from a card that is not there at all.
    pub fn save_game(&mut self, data: &SaveData) -> bool {
        self.status = SaveStatus::Saving;
        let mut target: Option<Card<HardwareCard>> = None;
        let mut unformatted_found = false;
        let mut unreadable_found = false;

        for slot in [Slot::One, Slot::Two] {
            let mut card = Card::new(HardwareCard::new(slot));
            match card.is_formatted() {
                Ok(true) => {}
                // SE-2: Never auto-format silently without user prompt to protect unseated/virgin cards
                Ok(false) => {
                    unformatted_found = true;
                    continue;
                }
                Err(psx_mc::Error::NoCard) => continue,
                // A protocol/transport fault is a hard error, not an absent
                // card: it used to be swallowed here and the player was told
                // "NO MEMORY CARD FOUND" about a card that was present.
                Err(_) => {
                    unreadable_found = true;
                    continue;
                }
            }
            // `is_formatted` only compares the two 'M'/'C' header bytes, so a
            // card with a damaged directory or a broken link chain passes the
            // probe and then fails mid-write. `validate_filesystem` is the
            // driver's strictest reachable check (header, every directory
            // checksum, allocation states, link chains, orphan blocks) and
            // performs no writes. It also requires Sony's convention that a
            // file's stored size equals its allocated blocks, so a card written
            // by a foreign tool can be reported unreadable here.
            if card.validate_filesystem().is_err() {
                unreadable_found = true;
                continue;
            }
            target = Some(card);
            break;
        }

        let Some(mut card) = target else {
            self.status = if unreadable_found {
                SaveStatus::SaveErrorCorrupt
            } else if unformatted_found {
                SaveStatus::SaveErrorUnformatted
            } else {
                SaveStatus::SaveErrorNoCard
            };
            self.status_timer = 120;
            return false;
        };

        let icon = get_platty_save_icon();
        // Refresh the checksum here: the game mutates its `SaveData` in place
        // between saves, so a checksum taken at construction is already stale.
        let payload = data.with_checksum().to_bytes();

        match card.write_with_icon(SAVE_FILENAME, SAVE_TITLE, &payload, &icon) {
            Ok(_) => {
                self.status = SaveStatus::SaveSuccess;
                self.last_load = LoadOutcome::Loaded;
                self.status_timer = 120;
                true
            }
            Err(psx_mc::Error::NoCard) => {
                self.status = SaveStatus::SaveErrorNoCard;
                self.status_timer = 120;
                false
            }
            Err(_) => {
                self.status = SaveStatus::SaveErrorFailed;
                self.status_timer = 120;
                false
            }
        }
    }

    /// Attempt to load game progress (queries Slot 1 first, falls back to Slot 2).
    pub fn load_from_slot1(&mut self) -> Option<SaveData> {
        self.load_game()
    }

    /// Load the save, distinguishing "no file", "file this build cannot use"
    /// and success. A file that is present but rejected is recorded in
    /// [`MemoryCardManager::last_load`] and reported as `LoadError` /
    /// `LoadIncompatible` instead of `LoadNotFound`, so it is never mistaken
    /// for an empty card and silently overwritten as a fresh campaign.
    pub fn load_game(&mut self) -> Option<SaveData> {
        self.status = SaveStatus::Loading;
        let mut rejected: Option<LoadOutcome> = None;
        let mut card_error = false;

        for slot in [Slot::One, Slot::Two] {
            let mut card = Card::new(HardwareCard::new(slot));
            let mut buf = [0u8; SAVE_BUFFER_SIZE];
            let outcome = match card.read(SAVE_FILENAME, &mut buf) {
                Ok(len) => classify_payload(&buf[..len]),
                // No file with our name here, or nothing in the port at all:
                // keep looking in the other slot, and conclude absence later.
                Err(psx_mc::Error::NotFound) | Err(psx_mc::Error::NoCard) => continue,
                // A card answered but its frames or transport failed: the card
                // is damaged, which is not the same as having no save.
                Err(psx_mc::Error::BadChecksum) | Err(psx_mc::Error::Protocol) => {
                    card_error = true;
                    continue;
                }
                // A file under our name that cannot be parsed or whose link
                // chain is broken: the file is unusable, not absent.
                Err(_) => {
                    rejected = Some(LoadOutcome::Corrupt);
                    continue;
                }
            };
            match outcome {
                LoadOutcome::Loaded => match SaveData::from_bytes(&buf[..SERIALIZED_SIZE]) {
                    Some(save) => {
                        self.last_load = LoadOutcome::Loaded;
                        self.status = SaveStatus::LoadSuccess;
                        self.status_timer = 90;
                        return Some(save);
                    }
                    // Cannot happen for a payload `classify_payload` accepted;
                    // treated as corruption rather than as an empty card.
                    None => rejected = Some(LoadOutcome::Corrupt),
                },
                other => {
                    // Keep looking in the other slot, but remember the
                    // rejection so a present file is never reported as absent.
                    if rejected.is_none() || rejected == Some(LoadOutcome::Incompatible) {
                        rejected = Some(other);
                    }
                }
            }
        }

        let outcome = match rejected {
            Some(o) => o,
            None if card_error => LoadOutcome::CardError,
            None => LoadOutcome::NotFound,
        };
        self.last_load = outcome;
        self.status = match outcome {
            LoadOutcome::Incompatible => SaveStatus::LoadIncompatible,
            LoadOutcome::NotFound => SaveStatus::LoadNotFound,
            _ => SaveStatus::LoadError,
        };
        self.status_timer = 90;
        None
    }
}
