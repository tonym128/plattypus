//! PlayStation 1 Memory Card manager, save data serialization,
//! and animated 16x16 BIOS save icon for Plattypus MGS.

use psx_mc::{Card, HardwareCard, SaveIcon, Slot};

pub use plattypus_core::save::*;

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
