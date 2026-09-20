//! PlayStation 1 Memory Card manager, save data serialization,
//! and animated 16x16 BIOS save icon for Plattypus MGS.

use psx_mc::{Card, HardwareCard, SaveIcon, Slot};

pub const SAVE_FILENAME: &str = "BASLUS-00001PLATTY";
pub const SAVE_TITLE: &str = "PLATTYPUS MGS";

const SAVE_MAGIC: [u8; 4] = *b"PLTY";
const SAVE_VERSION: u8 = 1;

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

    /// Attempt to save game progress to Slot 1 with BIOS animated icon.
    pub fn save_to_slot1(&mut self, data: &SaveData) -> bool {
        self.status = SaveStatus::Saving;
        let mut card = Card::new(HardwareCard::new(Slot::One));

        // Format if virgin card
        match card.is_formatted() {
            Ok(false) => {
                if card.format().is_err() {
                    self.status = SaveStatus::SaveErrorFailed;
                    self.status_timer = 120;
                    return false;
                }
            }
            Err(psx_mc::Error::NoCard) => {
                self.status = SaveStatus::SaveErrorNoCard;
                self.status_timer = 120;
                return false;
            }
            Err(_) => {
                self.status = SaveStatus::SaveErrorFailed;
                self.status_timer = 120;
                return false;
            }
            Ok(true) => {}
        }

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

    /// Attempt to load game progress from Slot 1.
    pub fn load_from_slot1(&mut self) -> Option<SaveData> {
        self.status = SaveStatus::Loading;
        let mut card = Card::new(HardwareCard::new(Slot::One));

        let mut buf = [0u8; 128];
        match card.read(SAVE_FILENAME, &mut buf) {
            Ok(len) if len >= core::mem::size_of::<SaveData>() => {
                let save = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const SaveData) };
                if save.is_valid() {
                    self.status = SaveStatus::LoadSuccess;
                    self.status_timer = 90;
                    Some(save)
                } else {
                    self.status = SaveStatus::LoadError;
                    self.status_timer = 120;
                    None
                }
            }
            Ok(_) => {
                self.status = SaveStatus::LoadError;
                self.status_timer = 120;
                None
            }
            Err(psx_mc::Error::NotFound) => {
                self.status = SaveStatus::LoadNotFound;
                self.status_timer = 90;
                None
            }
            Err(psx_mc::Error::NoCard) => {
                self.status = SaveStatus::SaveErrorNoCard;
                self.status_timer = 120;
                None
            }
            Err(_) => {
                self.status = SaveStatus::LoadError;
                self.status_timer = 120;
                None
            }
        }
    }
}
