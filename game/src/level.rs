//! 3D Tactical Stealth Environments for Plattypus MGS.
//! Full 3D sectors with cargo crates, security walls, air ducts, water canals,
//! searchlight posts, and infiltration exit burrows.

pub const GRID_W: usize = 24;
pub const GRID_D: usize = 24;
pub const TILE_SZ: i32 = 64; // 64x64 world units per tile

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
            CellType::Wall | CellType::Container => true,
            CellType::Crate => true,
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
        }
    }

    pub fn chapter(&self) -> u8 {
        match self {
            Act::Act1_1Drainage | Act::Act1_2Barracks | Act::Act1_3MechBoss => 1,
            Act::Act2_1Rapids | Act::Act2_2Mangroves | Act::Act2_3JetSkiBoss => 2,
            Act::Act3_1Highway | Act::Act3_2Laneways | Act::Act3_3SniperBoss => 3,
            Act::Act4_1Dunes | Act::Act4_2PierTrench | Act::Act4_3ExcavatorBoss => 4,
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
            _ => Act::Act4_3ExcavatorBoss,
        }
    }
}

pub struct Level {
    pub act: Act,
    pub cells: [CellType; GRID_W * GRID_D],
    pub player_start_x: i32,
    pub player_start_z: i32,
    pub exit_x: i32,
    pub exit_z: i32,
}

impl Level {
    pub fn new(act: Act) -> Self {
        let mut level = Self {
            act,
            cells: [CellType::Floor; GRID_W * GRID_D],
            player_start_x: 2 * TILE_SZ + 32,
            player_start_z: 2 * TILE_SZ + 32,
            exit_x: 21 * TILE_SZ + 32,
            exit_z: 21 * TILE_SZ + 32,
        };
        level.generate();
        level
    }

    #[inline]
    pub fn cell_index(gx: usize, gz: usize) -> usize {
        gz * GRID_W + gx
    }

    pub fn get_cell(&self, gx: usize, gz: usize) -> CellType {
        if gx >= GRID_W || gz >= GRID_D {
            CellType::Wall
        } else {
            self.cells[Self::cell_index(gx, gz)]
        }
    }

    pub fn set_cell(&mut self, gx: usize, gz: usize, cell: CellType) {
        if gx < GRID_W && gz < GRID_D {
            let idx = Self::cell_index(gx, gz);
            self.cells[idx] = cell;
        }
    }

    pub fn is_solid_at(&self, wx: i32, wz: i32, is_crawling: bool) -> bool {
        if wx < 0 || wz < 0 {
            return true;
        }
        let gx = (wx / TILE_SZ) as usize;
        let gz = (wz / TILE_SZ) as usize;
        self.get_cell(gx, gz).is_solid(is_crawling)
    }

    pub fn is_water_at(&self, wx: i32, wz: i32) -> bool {
        if wx < 0 || wz < 0 {
            return false;
        }
        let gx = (wx / TILE_SZ) as usize;
        let gz = (wz / TILE_SZ) as usize;
        self.get_cell(gx, gz).is_water()
    }

    pub fn is_tall_grass_at(&self, wx: i32, wz: i32) -> bool {
        if wx < 0 || wz < 0 {
            return false;
        }
        let gx = (wx / TILE_SZ) as usize;
        let gz = (wz / TILE_SZ) as usize;
        self.get_cell(gx, gz) == CellType::TallGrass
    }

    pub fn is_exit_at(&self, wx: i32, wz: i32) -> bool {
        let dx = (wx - self.exit_x).abs();
        let dz = (wz - self.exit_z).abs();
        dx < 40 && dz < 40
    }

    fn generate(&mut self) {
        // Enclosing boundary perimeter walls
        for x in 0..GRID_W {
            self.set_cell(x, 0, CellType::Wall);
            self.set_cell(x, GRID_D - 1, CellType::Wall);
        }
        for z in 0..GRID_D {
            self.set_cell(0, z, CellType::Wall);
            self.set_cell(GRID_W - 1, z, CellType::Wall);
        }

        match self.act {
            Act::Act1_1Drainage => self.generate_act1_1(),
            Act::Act1_2Barracks => self.generate_act1_2(),
            Act::Act1_3MechBoss => self.generate_act1_3(),
            Act::Act2_1Rapids => self.generate_act2_1(),
            Act::Act2_2Mangroves => self.generate_act2_2(),
            Act::Act2_3JetSkiBoss => self.generate_act2_3(),
            Act::Act3_1Highway => self.generate_act3_1(),
            Act::Act3_2Laneways => self.generate_act3_2(),
            Act::Act3_3SniperBoss => self.generate_act3_3(),
            Act::Act4_1Dunes => self.generate_act4_1(),
            Act::Act4_2PierTrench => self.generate_act4_2(),
            Act::Act4_3ExcavatorBoss => self.generate_act4_3(),
        }
    }

    // -------------------------------------------------------------------------
    // ACT 1-1: HEALESVILLE SANCTUARY (SECURITY DRAINAGE OUTFLOW)
    // -------------------------------------------------------------------------
    fn generate_act1_1(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 2 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 21 * TILE_SZ + 32;

        // Security perimeter dividing fence with air duct shortcut
        for z in 2..16 {
            self.set_cell(8, z, CellType::Wall);
        }
        self.set_cell(8, 8, CellType::AirDuct); // Secret crawl tunnel through wall!

        // Drainage sluice canal cutting diagonally across
        for x in 4..12 {
            self.set_cell(x, 14, CellType::Water);
        }
        for z in 14..22 {
            self.set_cell(12, z, CellType::Water);
        }

        // Inner security compound walls
        for x in 12..22 {
            self.set_cell(x, 10, CellType::Wall);
        }
        self.set_cell(16, 10, CellType::LaserTripwire); // Security beam
        self.set_cell(18, 10, CellType::AirDuct);       // Vent pipe

        // Cargo crates providing tactical cover
        let crates = [
            (3, 4), (4, 4), (5, 6), (3, 9), (4, 9),
            (10, 4), (11, 4), (14, 5), (15, 5),
            (6, 17), (7, 17), (9, 19), (10, 19),
            (16, 14), (17, 14), (18, 16), (19, 16),
            (15, 20), (16, 20),
        ];
        for (cx, cz) in crates {
            self.set_cell(cx, cz, CellType::Crate);
        }

        // Camouflage tall grass patches near the water
        let grass = [
            (2, 12), (3, 12), (3, 13),
            (10, 12), (11, 12), (11, 13),
            (13, 17), (14, 17), (14, 18),
        ];
        for (gx, gz) in grass {
            self.set_cell(gx, gz, CellType::TallGrass);
        }

        // Acoustic metal catwalks across guard patrol routes (loud footsteps when running!)
        for z in 6..10 {
            self.set_cell(5, z, CellType::MetalGrate);
        }
        for x in 16..20 {
            self.set_cell(x, 4, CellType::MetalGrate);
        }

        // Exit burrow in far corner
        self.set_cell(21, 21, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 1-2: RESEARCH BARRACKS & LASER GRID MAZE
    // -------------------------------------------------------------------------
    fn generate_act1_2(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Interior concrete dividing walls creating 4 security sectors
        for z in 2..22 {
            if z != 6 && z != 16 {
                self.set_cell(7, z, CellType::Wall);
            }
            if z != 11 {
                self.set_cell(15, z, CellType::Wall);
            }
        }
        for x in 2..22 {
            if x != 11 {
                self.set_cell(x, 11, CellType::Wall);
            }
        }

        // Laser tripwires across high-security hallways
        self.set_cell(7, 6, CellType::LaserTripwire);
        self.set_cell(7, 16, CellType::LaserTripwire);
        self.set_cell(15, 11, CellType::LaserTripwire);
        self.set_cell(11, 11, CellType::LaserTripwire);
        self.set_cell(11, 6, CellType::LaserTripwire);
        self.set_cell(11, 16, CellType::LaserTripwire);

        // Ventilation duct crawl passages connecting sectors (stealth bypasses!)
        self.set_cell(7, 4, CellType::AirDuct);
        self.set_cell(7, 18, CellType::AirDuct);
        self.set_cell(15, 8, CellType::AirDuct);
        self.set_cell(15, 14, CellType::AirDuct);
        self.set_cell(11, 4, CellType::AirDuct);
        self.set_cell(11, 18, CellType::AirDuct);

        // Barracks furniture, computer servers, and supply crates
        let crates = [
            (3, 4), (4, 4), (3, 7), (4, 7), (3, 14), (4, 14), (3, 17), (4, 17),
            (10, 3), (10, 4), (12, 3), (12, 4), (10, 18), (10, 19), (12, 18), (12, 19),
            (18, 5), (19, 5), (18, 8), (19, 8), (18, 14), (19, 14), (18, 17), (19, 17),
        ];
        for (cx, cz) in crates {
            self.set_cell(cx, cz, CellType::Crate);
        }

        // Acoustic metal grating in central guard patrol corridors
        for z in 5..=17 {
            self.set_cell(3, z, CellType::MetalGrate);
            self.set_cell(11, z, CellType::MetalGrate);
            self.set_cell(19, z, CellType::MetalGrate);
        }

        // Exit blast door burrow leading out to perimeter wall
        self.set_cell(21, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 1-3: PERIMETER WALL (SEARCHLIGHT MECH BOSS)
    // -------------------------------------------------------------------------
    fn generate_act1_3(&mut self) {
        self.player_start_x = 12 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 12 * TILE_SZ + 32;
        self.exit_z = 1 * TILE_SZ + 32;

        // North perimeter security wall with central heavy blast gate
        for x in 1..GRID_W - 1 {
            self.set_cell(x, 1, CellType::Wall);
            self.set_cell(x, 2, CellType::Wall);
        }
        // Blast gate in center initially locked
        self.set_cell(11, 1, CellType::Wall);
        self.set_cell(12, 1, CellType::Wall);
        self.set_cell(13, 1, CellType::Wall);
        self.set_cell(11, 2, CellType::Wall);
        self.set_cell(12, 2, CellType::Wall);
        self.set_cell(13, 2, CellType::Wall);

        // Three deep drainage crawl trenches (crawling protects Platty from sweeps and stomps)
        // 1. West generator trench (gx 3..6, gz 8..10)
        for z in 8..=10 {
            for x in 3..=6 {
                self.set_cell(x, z, CellType::AirDuct);
            }
        }

        // 2. East generator trench (gx 17..20, gz 8..10)
        for z in 8..=10 {
            for x in 17..=20 {
                self.set_cell(x, z, CellType::AirDuct);
            }
        }

        // 3. South generator trench (gx 9..14, gz 16..17)
        for z in 16..=17 {
            for x in 9..=14 {
                self.set_cell(x, z, CellType::AirDuct);
            }
        }

        // Concrete cover crates and blast barriers
        let cover_crates = [
            (5, 5), (6, 5), (17, 5), (18, 5),
            (8, 11), (9, 11), (14, 11), (15, 11),
            (5, 14), (6, 14), (17, 14), (18, 14),
            (11, 13), (12, 13),
        ];
        for (cx, cz) in cover_crates {
            self.set_cell(cx, cz, CellType::Crate);
        }

        // Camouflage tall grass patches near trenches
        let grass = [
            (7, 9), (7, 10),
            (16, 9), (16, 10),
            (8, 17), (15, 17),
        ];
        for (gx, gz) in grass {
            self.set_cell(gx, gz, CellType::TallGrass);
        }
    }

    // -------------------------------------------------------------------------
    // ACT 2-1: UPPER GORGE RAPIDS (5-LANE RIVER RUNNER)
    // -------------------------------------------------------------------------
    fn generate_act2_1(&mut self) {
        self.player_start_x = 11 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 11 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // 5-lane rushing river corridor (Lanes 0..4 = gx 9, 10, 11, 12, 13)
        for z in 1..23 {
            // Left forested bank
            for x in 1..9 {
                self.set_cell(x, z, CellType::Wall);
            }
            // 5 rushing water flume lanes
            for x in 9..=13 {
                self.set_cell(x, z, CellType::WaterCurrent);
            }
            // Right forested bank
            for x in 14..23 {
                self.set_cell(x, z, CellType::Wall);
            }
        }

        // River exit flume leading into mangrove backwaters
        self.set_cell(11, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 2-2: DANDENONG MURKY MANGROVES & CAVERN MAZE
    // -------------------------------------------------------------------------
    fn generate_act2_2(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 2 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 21 * TILE_SZ + 32;

        // Swamp islands and root walls
        for z in 4..19 {
            if z != 8 && z != 14 {
                self.set_cell(6, z, CellType::Wall);
                self.set_cell(12, z, CellType::Wall);
                self.set_cell(17, z, CellType::Wall);
            }
        }

        // Winding murky water channels
        for z in 1..23 {
            for x in 1..23 {
                if (x + z) % 3 == 0 || (x % 5 == 0) {
                    if self.get_cell(x, z) != CellType::Wall {
                        self.set_cell(x, z, CellType::Water);
                    }
                }
            }
        }

        // Hollow log crawl tunnels beneath tangled mangrove root systems
        self.set_cell(6, 8, CellType::AirDuct);
        self.set_cell(6, 14, CellType::AirDuct);
        self.set_cell(12, 8, CellType::AirDuct);
        self.set_cell(12, 14, CellType::AirDuct);
        self.set_cell(17, 8, CellType::AirDuct);
        self.set_cell(17, 14, CellType::AirDuct);

        // Fallen mossy logs
        let logs = [
            (3, 5), (4, 5), (9, 7), (10, 7), (14, 4), (15, 4),
            (3, 16), (4, 16), (9, 17), (10, 17), (14, 18), (15, 18),
            (8, 11), (9, 11), (14, 11), (15, 11),
        ];
        for (lx, lz) in logs {
            self.set_cell(lx, lz, CellType::Crate);
        }

        // Dense tall swamp reeds providing sonar concealment
        for z in 2..22 {
            for x in 2..22 {
                if (x * 7 + z * 13) % 9 == 0 && self.get_cell(x, z) == CellType::Floor {
                    self.set_cell(x, z, CellType::TallGrass);
                }
            }
        }

        // Exit burrow leading into the rapids pursuit channel
        self.set_cell(21, 21, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 2-3: RIVER RAPIDS PURSUIT (PARK RANGER JET SKI BOSS)
    // -------------------------------------------------------------------------
    fn generate_act2_3(&mut self) {
        self.player_start_x = 11 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 11 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Wide 6-lane rushing river canyon (gx 8..=15)
        for z in 1..23 {
            // Left canyon cliff
            for x in 1..8 {
                self.set_cell(x, z, CellType::Wall);
            }
            // 6-lane rushing water
            for x in 8..=15 {
                self.set_cell(x, z, CellType::WaterCurrent);
            }
            // Right canyon cliff
            for x in 16..23 {
                self.set_cell(x, z, CellType::Wall);
            }
        }

        // Exit flume into city waterways
        self.set_cell(11, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 3-1: MELBOURNE DOWNTOWN (CITY FROGGER)
    // -------------------------------------------------------------------------
    fn generate_act3_1(&mut self) {
        self.player_start_x = 12 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 12 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Flanking illuminated skyscrapers
        for z in 1..23 {
            for x in 1..4 {
                self.set_cell(x, z, CellType::Container); // West tower blocks
            }
            for x in 20..23 {
                self.set_cell(x, z, CellType::Container); // East tower blocks
            }
        }

        // Central Park median strip with trees and grass
        for x in 4..20 {
            self.set_cell(x, 15, CellType::TallGrass);
            self.set_cell(x, 16, CellType::TallGrass);
            self.set_cell(x, 9, CellType::TallGrass);
        }

        // Destination: Laneways entrance at north end
        self.set_cell(12, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 3-2: FLINDERS STREET LANEWAYS & ROOFTOP CATWALKS
    // -------------------------------------------------------------------------
    fn generate_act3_2(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Brick buildings creating narrow Melbourne graffiti alleyways
        for z in 3..21 {
            if z != 7 && z != 15 {
                self.set_cell(6, z, CellType::Container);
                self.set_cell(12, z, CellType::Container);
                self.set_cell(17, z, CellType::Container);
            }
        }

        // Dumpsters, trash bins, and delivery crates
        let bins = [
            (3, 5), (4, 5), (3, 12), (4, 12), (3, 18), (4, 18),
            (9, 4), (10, 4), (9, 13), (10, 13), (9, 19), (10, 19),
            (14, 6), (15, 6), (14, 16), (15, 16),
            (19, 4), (20, 4), (19, 12), (20, 12), (19, 18), (20, 18),
        ];
        for (bx, bz) in bins {
            self.set_cell(bx, bz, CellType::Crate);
        }

        // Acoustic metal fire escapes & overhead catwalks
        for z in 4..19 {
            self.set_cell(4, z, CellType::MetalGrate);
            self.set_cell(10, z, CellType::MetalGrate);
            self.set_cell(15, z, CellType::MetalGrate);
        }

        // Low basement air vents connecting the laneways
        self.set_cell(6, 7, CellType::AirDuct);
        self.set_cell(6, 15, CellType::AirDuct);
        self.set_cell(12, 7, CellType::AirDuct);
        self.set_cell(12, 15, CellType::AirDuct);
        self.set_cell(17, 7, CellType::AirDuct);
        self.set_cell(17, 15, CellType::AirDuct);

        // Antenna tower access ladder burrow
        self.set_cell(21, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 3-3: ANTENNA TOWER & SNIPER KOOKABURRA BOSS
    // -------------------------------------------------------------------------
    fn generate_act3_3(&mut self) {
        self.player_start_x = 12 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 12 * TILE_SZ + 32;
        self.exit_z = 1 * TILE_SZ + 32;

        // North edge rooftop edge wall
        for x in 1..GRID_W - 1 {
            self.set_cell(x, 1, CellType::Wall);
            self.set_cell(x, 2, CellType::Wall);
        }
        self.set_cell(11, 1, CellType::Wall);
        self.set_cell(12, 1, CellType::Wall);

        // 3 Large Broadcast Transmission Towers (perches for Sniper Kookaburra)
        // 0: West tower (gx 4, gz 8)
        self.set_cell(4, 7, CellType::Container);
        self.set_cell(4, 8, CellType::Container);
        // 1: North tower (gx 12, gz 5)
        self.set_cell(12, 4, CellType::Container);
        self.set_cell(12, 5, CellType::Container);
        // 2: East tower (gx 19, gz 8)
        self.set_cell(19, 7, CellType::Container);
        self.set_cell(19, 8, CellType::Container);

        // Heavy rooftop air conditioning chiller units (sniper cover)
        let ac_units = [
            (7, 10), (8, 10), (15, 10), (16, 10),
            (8, 15), (9, 15), (14, 15), (15, 15),
            (11, 12), (12, 12),
        ];
        for (cx, cz) in ac_units {
            self.set_cell(cx, cz, CellType::Crate);
        }

        // Low ventilation ducts beneath chillers (duck to avoid sniper crosshairs!)
        self.set_cell(7, 11, CellType::AirDuct);
        self.set_cell(16, 11, CellType::AirDuct);
        self.set_cell(8, 16, CellType::AirDuct);
        self.set_cell(15, 16, CellType::AirDuct);
        self.set_cell(11, 13, CellType::AirDuct);
        self.set_cell(12, 13, CellType::AirDuct);

        // Acoustic metal grating surrounding generator pads
        for z in 13..=14 {
            for x in 9..=14 {
                self.set_cell(x, z, CellType::MetalGrate);
            }
        }
    }

    // -------------------------------------------------------------------------
    // ACT 4-1: COASTAL DUNES & SURF (3D PLATFORMER)
    // -------------------------------------------------------------------------
    fn generate_act4_1(&mut self) {
        self.player_start_x = 4 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 20 * TILE_SZ + 32;
        self.exit_z = 3 * TILE_SZ + 32;

        // Ocean surf and deep water along southwest edge
        for z in 15..23 {
            for x in 1..3 {
                self.set_cell(x, z, CellType::Water);
            }
        }

        // Stepped rock cliffs & platforms to climb
        let rock_cliffs = [
            (6, 12), (7, 12), (8, 12),
            (10, 8), (11, 8), (12, 8),
            (14, 14), (15, 14), (16, 14),
            (17, 9), (18, 9), (19, 9),
            (12, 4), (13, 4), (14, 4),
            (18, 4), (19, 4), (20, 4),
        ];
        for (rx, rz) in rock_cliffs {
            self.set_cell(rx, rz, CellType::Container);
        }

        // Coastal dune grass
        let grass = [
            (4, 18), (5, 18), (8, 17),
            (11, 13), (12, 13), (16, 11),
            (15, 6), (16, 6),
        ];
        for (gx, gz) in grass {
            self.set_cell(gx, gz, CellType::TallGrass);
        }

        // Pier access burrow atop the dunes
        self.set_cell(20, 3, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 4-2: PIER UNDERSTRUCTURE & DEEP WATER SHARK TRENCH
    // -------------------------------------------------------------------------
    fn generate_act4_2(&mut self) {
        self.player_start_x = 3 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Timber pier pilings supporting overhead deck
        for z in 2..22 {
            if z % 3 == 0 {
                self.set_cell(5, z, CellType::Wall);
                self.set_cell(10, z, CellType::Wall);
                self.set_cell(15, z, CellType::Wall);
            }
        }

        // Deep water channels throughout understructure
        for z in 1..23 {
            for x in 1..23 {
                if self.get_cell(x, z) != CellType::Wall {
                    self.set_cell(x, z, CellType::Water);
                }
            }
        }

        // Floating wooden supply rafts providing dry rest spots
        let rafts = [
            (3, 18), (4, 18), (3, 10), (4, 10),
            (7, 14), (8, 14), (7, 6), (8, 6),
            (12, 16), (13, 16), (12, 8), (13, 8),
            (17, 12), (18, 12), (17, 4), (18, 4),
        ];
        for (rx, rz) in rafts {
            self.set_cell(rx, rz, CellType::Floor);
        }

        // Submerged barnacle crawl archways
        self.set_cell(5, 7, CellType::AirDuct);
        self.set_cell(5, 14, CellType::AirDuct);
        self.set_cell(10, 7, CellType::AirDuct);
        self.set_cell(10, 14, CellType::AirDuct);
        self.set_cell(15, 7, CellType::AirDuct);
        self.set_cell(15, 14, CellType::AirDuct);

        // Submerged coastal cavern burrow leading into nursery cove
        self.set_cell(21, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 4-3: BURROW DEFENSE & DR. CANE TOAD'S EXCAVATOR (FINAL CLIMAX)
    // -------------------------------------------------------------------------
    fn generate_act4_3(&mut self) {
        self.player_start_x = 12 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 12 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // Sandstone coastal bluffs surrounding nursery cove
        for z in 1..8 {
            for x in 1..8 {
                self.set_cell(x, z, CellType::Wall);
            }
            for x in 16..23 {
                self.set_cell(x, z, CellType::Wall);
            }
        }

        // Pip's nursery burrow nestled into the cliffside
        self.set_cell(11, 2, CellType::Wall);
        self.set_cell(12, 2, CellType::ExitBurrow);
        self.set_cell(13, 2, CellType::Wall);

        // Mud drainage crawl trenches (crawling dodges sweeping shovel arm)
        for z in 14..=15 {
            for x in 4..=9 {
                self.set_cell(x, z, CellType::AirDuct);
            }
            for x in 14..=19 {
                self.set_cell(x, z, CellType::AirDuct);
            }
        }

        // Supply crates and sandbags
        let crates = [
            (4, 10), (5, 10), (18, 10), (19, 10),
            (9, 17), (10, 17), (13, 17), (14, 17),
        ];
        for (cx, cz) in crates {
            self.set_cell(cx, cz, CellType::Crate);
        }

        // Tall coastal beach grass
        let grass = [
            (6, 12), (7, 12), (16, 12), (17, 12),
            (10, 19), (11, 19), (12, 19), (13, 19),
        ];
        for (gx, gz) in grass {
            self.set_cell(gx, gz, CellType::TallGrass);
        }
    }
}
