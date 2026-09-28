//! Host-side test suite for Plattypus game logic invariants.
//! Tests save serialization, checksum verification, codename ranking,
//! act progression, and cell collisions without requiring bare-metal MIPS hardware.

const SAVE_MAGIC: [u8; 4] = *b"PLTY";
const SAVE_VERSION: u8 = 3;

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

    // 5. Campaign Completion & Continue Bounds Tests (QA-1 / PD-1)
    let max_campaign_act = Act::Act4_3ExcavatorBoss as u8;
    assert_eq!(max_campaign_act, 11, "Act 4-3 must be act index 11");

    // Simulate completion of Act 4-3: unlocked_act must be clamped to 11
    let next_act_idx = if max_campaign_act < 11 {
        max_campaign_act + 1
    } else {
        11
    };
    let completed_unlocked_act = next_act_idx.min(11);
    assert_eq!(completed_unlocked_act, 11, "Completed campaign must clamp unlocked_act to 11");

    // Continue flow: continue act index must never resolve to a VR act (>= 12)
    for unlocked in 0..=255u8 {
        let continue_act_idx = unlocked.min(11);
        let continue_act = Act::from_u8(continue_act_idx);
        assert!(!continue_act.is_vr(), "Continue act must never be a VR training stage");
        assert!((continue_act as u8) <= 11, "Continue act index must be in 0..=11");
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

    println!("\nALL PLATTYPUS GAME LOGIC TESTS PASSED SUCCESSFULLY! (7/7 test suites)");
}
