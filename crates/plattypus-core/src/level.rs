// SPDX-License-Identifier: GPL-2.0-or-later
//! Shared level, act, and grid definitions for Plattypus. Pure no_std.

pub const GRID_W: usize = 24;
pub const GRID_D: usize = 24;
pub const TILE_SZ: i32 = 64;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CellType {
    Floor = 0,         // Open walkable ground
    Wall = 1,          // High security concrete perimeter wall
    Crate = 2,         // Cargo cover crate ("MELBOURNE FRUIT CO")
    Container = 3,     // Large military freight container
    Water = 4,         // Water canal (swim & submerge)
    WaterCurrent = 5,  // Fast water flume pushing downstream
    AirDuct = 6,       // Low ventilation shaft (must crawl to pass)
    TallGrass = 7,     // Australian bush scrub (crouch camouflage)
    LaserTripwire = 8, // Security laser tripwire
    ExitBurrow = 9,    // Stage exit infiltration burrow
    MetalGrate = 10,   // Loud acoustic walkway / catwalk
}

impl CellType {
    #[inline]
    pub fn is_solid(&self, is_crawling: bool) -> bool {
        match self {
            CellType::Wall | CellType::Container | CellType::Crate => true,
            CellType::AirDuct => !is_crawling, // Passable only when crawling!
            _ => false,
        }
    }

    #[inline]
    pub fn is_water(&self) -> bool {
        matches!(self, CellType::Water | CellType::WaterCurrent)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Act {
    // Chapter 1: The Healesville Sanctuary
    Act1_1Drainage = 0,
    Act1_2Barracks = 1,
    Act1_3MechBoss = 2,

    // Chapter 2: The Yarra River Wilds
    Act2_1Rapids = 3,
    Act2_2Mangroves = 4,
    Act2_3JetSkiBoss = 5,

    // Chapter 3: Melbourne Downtown
    Act3_1Highway = 6,
    Act3_2Laneways = 7,
    Act3_3SniperBoss = 8,

    // Chapter 4: Coastal Beachhead
    Act4_1Dunes = 9,
    Act4_2PierTrench = 10,
    Act4_3ExcavatorBoss = 11,

    // VR Training Simulator Stages
    VrSneaking = 12,
    VrCqc = 13,
    VrSonar = 14,
    VrSpeed = 15,
}

impl Act {
    pub fn title(&self) -> &'static str {
        match self {
            Act::Act1_1Drainage => "ACT 1-1: DRAINAGE OUTFLOW (STEALTH)",
            Act::Act1_2Barracks => "ACT 1-2: RESEARCH BARRACKS (STEALTH MAZE)",
            Act::Act1_3MechBoss => "ACT 1-3: PERIMETER WALL (SEARCHLIGHT MECH)",
            Act::Act2_1Rapids => "ACT 2-1: UPPER GORGE RAPIDS (RIVER RUNNER)",
            Act::Act2_2Mangroves => "ACT 2-2: DANDENONG MANGROVES (SONAR MAZE)",
            Act::Act2_3JetSkiBoss => "ACT 2-3: RIVER PURSUIT (JET SKI BOSS)",
            Act::Act3_1Highway => "ACT 3-1: NEON HIGHWAY (CITY FROGGER)",
            Act::Act3_2Laneways => "ACT 3-2: FLINDERS LANEWAYS (URBAN STEALTH)",
            Act::Act3_3SniperBoss => "ACT 3-3: ANTENNA TOWER (SNIPER KOOKY)",
            Act::Act4_1Dunes => "ACT 4-1: COASTAL DUNES (3D PLATFORMER)",
            Act::Act4_2PierTrench => "ACT 4-2: PIER UNDERSTRUCTURE (SHARK TRENCH)",
            Act::Act4_3ExcavatorBoss => "ACT 4-3: BURROW DEFENSE (EXCAVATOR CLIMAX)",
            Act::VrSneaking => "VR-01: SNEAKING SIMULATOR",
            Act::VrCqc => "VR-02: CQC SPUR TAKEDOWN",
            Act::VrSonar => "VR-03: SONAR LABYRINTH",
            Act::VrSpeed => "VR-04: SPEED HURDLES",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Act::Act1_1Drainage => "Infiltrate past guards, crawl through air vents, and escape the canal!",
            Act::Act1_2Barracks => "Navigate guard barracks, dodge laser tripwires, and use the cardboard box!",
            Act::Act1_3MechBoss => "Disable 3 power conduits, dodge dual searchlights, and strike the Mech!",
            Act::Act2_1Rapids => "Surf 5 river lanes! Dodge tubers, boarders, trees, and snakes!",
            Act::Act2_2Mangroves => "Murky backwaters! Use electro-sonar to spot submerged roots & spiders!",
            Act::Act2_3JetSkiBoss => "High-speed pursuit! Evade floating water mines and strike the engine!",
            Act::Act3_1Highway => "Cross multi-lane rush hour avenues! Dodge taxis, trams, & trucks!",
            Act::Act3_2Laneways => "Sneak through trash alleys & rooftop catwalks! Beware acoustic grates!",
            Act::Act3_3SniperBoss => "Scale the broadcast antennas! Avoid laser targeting and spur strike Kooky!",
            Act::Act4_1Dunes => "Platform across dunes & bounce on parasols to reach baby sister Pip!",
            Act::Act4_2PierTrench => "Deep ocean surf! Submerge under shark patrols to find the cavern tunnel!",
            Act::Act4_3ExcavatorBoss => "Protect Pip's nursery! Overload Dr. Cane Toad's amphibian excavator!",
            Act::VrSneaking => "Evade virtual patrol drones and reach the exit burrow undetected!",
            Act::VrCqc => "Sneak behind unalerted guards and neutralize all targets with CQC spurs!",
            Act::VrSonar => "Navigate the pitch black submerged maze using electro-sonar pulses!",
            Act::VrSpeed => "Speed sprint through floating platforms & obstacles in under 30 seconds!",
        }
    }

    pub fn stage_label(&self) -> &'static str {
        match self {
            Act::Act1_1Drainage => "STAGE 1-1",
            Act::Act1_2Barracks => "STAGE 1-2",
            Act::Act1_3MechBoss => "STAGE 1-3",
            Act::Act2_1Rapids => "STAGE 2-1",
            Act::Act2_2Mangroves => "STAGE 2-2",
            Act::Act2_3JetSkiBoss => "STAGE 2-3",
            Act::Act3_1Highway => "STAGE 3-1",
            Act::Act3_2Laneways => "STAGE 3-2",
            Act::Act3_3SniperBoss => "STAGE 3-3",
            Act::Act4_1Dunes => "STAGE 4-1",
            Act::Act4_2PierTrench => "STAGE 4-2",
            Act::Act4_3ExcavatorBoss => "STAGE 4-3",
            Act::VrSneaking => "VR-01",
            Act::VrCqc => "VR-02",
            Act::VrSonar => "VR-03",
            Act::VrSpeed => "VR-04",
        }
    }

    pub fn chapter(&self) -> u8 {
        match self {
            Act::Act1_1Drainage | Act::Act1_2Barracks | Act::Act1_3MechBoss => 1,
            Act::Act2_1Rapids | Act::Act2_2Mangroves | Act::Act2_3JetSkiBoss => 2,
            Act::Act3_1Highway | Act::Act3_2Laneways | Act::Act3_3SniperBoss => 3,
            Act::Act4_1Dunes | Act::Act4_2PierTrench | Act::Act4_3ExcavatorBoss => 4,
            Act::VrSneaking | Act::VrCqc | Act::VrSonar | Act::VrSpeed => 5,
        }
    }

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

    pub fn from_index(idx: usize) -> Self {
        Self::from_u8(idx as u8)
    }

    pub fn index(&self) -> usize {
        *self as usize
    }
}
