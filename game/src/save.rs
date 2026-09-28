//! PlayStation 1 Memory Card manager, save data serialization,
//! and animated 16x16 BIOS save icon for Plattypus MGS.

use psx_mc::{Card, HardwareCard, SaveIcon, Slot};

pub const SAVE_FILENAME: &str = "BASLUS-00001PLATTY";
pub const SAVE_TITLE: &str = "PLATTYPUS MGS";

const SAVE_MAGIC: [u8; 4] = *b"PLTY";
const SAVE_VERSION: u8 = 3;

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

/// Auto-detect PlayStation hardware video standard (PAL 50Hz vs NTSC 60Hz)
/// by inspecting the system ROM date string in the BIOS.
pub fn detect_console_region() -> (psx_gpu::VideoMode, &'static str) {
    let region_char = unsafe {
        let ptr = 0xBFC7FF52 as *const u8;
        *ptr
    };
    match region_char {
        b'E' => (psx_gpu::VideoMode::Pal, "PAL (EUROPE / 50Hz)"),
        b'J' => (psx_gpu::VideoMode::Ntsc, "NTSC-J (JAPAN / 60Hz)"),
        b'A' => (psx_gpu::VideoMode::Ntsc, "NTSC-U/C (NORTH AMERICA / 60Hz)"),
        _ => (psx_gpu::VideoMode::Ntsc, "NTSC (STANDARD / 60Hz)"),
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

    pub fn compute_checksum(&self) -> u16 {
        let mut sum: u16 = 0x5A5A;
        sum = sum.wrapping_add(self.version as u16);
        sum = sum.wrapping_add(self.unlocked_act as u16);
        sum = sum.wrapping_add((self.highest_score & 0xFFFF) as u16);
        sum = sum.wrapping_add((self.highest_score >> 16) as u16);
        sum = sum.wrapping_add(self.total_yabbies);
        sum = sum.wrapping_add(self.alerts_count);
        sum = sum.wrapping_add((self.best_time_seconds & 0xFFFF) as u16);
        for b in self.best_codename.iter() {
            sum = sum.wrapping_add(*b as u16);
        }
        sum = sum.wrapping_add(self.tuxedo_unlocked as u16);
        sum = sum.wrapping_add(self.camo_unlocked as u16);
        sum = sum.wrapping_add(self.wireframe_unlocked as u16);
        sum = sum.wrapping_add(self.vr_cleared as u16);
        sum = sum.wrapping_add(self.selected_costume as u16);
        sum = sum.wrapping_add(self.wireframe_enabled as u16);
        sum = sum.wrapping_add(self.language as u16);
        sum = sum.wrapping_add(self.screen_offset_x as u8 as u16);
        sum = sum.wrapping_add(self.screen_offset_y as u8 as u16);
        sum = sum.wrapping_add(self.pal_mode as u16);
        sum
    }

    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.version == SAVE_VERSION && self.checksum == self.compute_checksum()
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
    SaveErrorFailed,
    Loading,
    LoadSuccess,
    LoadNotFound,
    LoadError,
}

pub struct MemoryCardManager {
    pub status: SaveStatus,
    pub status_timer: u16,
}

impl MemoryCardManager {
    pub fn new() -> Self {
        Self {
            status: SaveStatus::Idle,
            status_timer: 0,
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

    pub fn save_game(&mut self, data: &SaveData) -> bool {
        self.status = SaveStatus::Saving;
        let slots = [Slot::One, Slot::Two];
        let mut target_slot = None;
        let mut unformatted_found = false;

        for &slot in &slots {
            let mut card = Card::new(HardwareCard::new(slot));
            match card.is_formatted() {
                Ok(true) => {
                    target_slot = Some(slot);
                    break;
                }
                Ok(false) => {
                    // SE-2: Never auto-format silently without user prompt to protect unseated/virgin cards
                    unformatted_found = true;
                }
                Err(psx_mc::Error::NoCard) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            }
        }

        let slot = match target_slot {
            Some(s) => s,
            None => {
                if unformatted_found {
                    self.status = SaveStatus::SaveErrorUnformatted;
                } else {
                    self.status = SaveStatus::SaveErrorNoCard;
                }
                self.status_timer = 120;
                return false;
            }
        };

        let mut card = Card::new(HardwareCard::new(slot));
        let icon = get_platty_save_icon();
        let payload = unsafe {
            core::slice::from_raw_parts(
                data as *const SaveData as *const u8,
                core::mem::size_of::<SaveData>(),
            )
        };

        match card.write_with_icon(SAVE_FILENAME, SAVE_TITLE, payload, &icon) {
            Ok(_) => {
                self.status = SaveStatus::SaveSuccess;
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

    pub fn load_game(&mut self) -> Option<SaveData> {
        self.status = SaveStatus::Loading;
        let slots = [Slot::One, Slot::Two];

        for &slot in &slots {
            let mut card = Card::new(HardwareCard::new(slot));
            let mut buf = [0u8; 128];
            match card.read(SAVE_FILENAME, &mut buf) {
                Ok(len) if len >= core::mem::size_of::<SaveData>() => {
                    let save = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const SaveData) };
                    if save.is_valid() {
                        self.status = SaveStatus::LoadSuccess;
                        self.status_timer = 90;
                        return Some(save);
                    }
                }
                _ => {}
            }
        }

        self.status = SaveStatus::LoadNotFound;
        self.status_timer = 90;
        None
    }
}
