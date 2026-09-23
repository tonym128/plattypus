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
pub enum Act {
    Act1Sanctuary,
    Act1Boss,
    Act2Bushland,
    Act3City,
    Act4Ocean,
}

impl Act {
    pub fn title(&self) -> &'static str {
        match self {
            Act::Act1Sanctuary => "ACT 1: HEALESVILLE COMPOUND (STEALTH)",
            Act::Act1Boss => "ACT 1 CLIMAX: PERIMETER WALL (BOSS)",
            Act::Act2Bushland => "ACT 2: YARRA RIVER RAPIDS (RUNNER)",
            Act::Act3City => "ACT 3: MELBOURNE DOWNTOWN (FROGGER)",
            Act::Act4Ocean => "ACT 4: COASTAL DUNES & SURF (PLATFORMER)",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Act::Act1Sanctuary => "Infiltrate past guards, crawl through air vents, and escape!",
            Act::Act1Boss => "Disable 3 power conduits, dodge dual searchlights, and strike the Mech!",
            Act::Act2Bushland => "Surf 5 river lanes! Dodge tubers, boarders, trees, and snakes!",
            Act::Act3City => "Cross multi-lane rush hour avenues! Dodge taxis, trams, & trucks!",
            Act::Act4Ocean => "Platform across dunes & bounce on parasols to reach baby sister Pip!",
        }
    }

    pub fn next(&self) -> Option<Act> {
        match self {
            Act::Act1Sanctuary => Some(Act::Act1Boss),
            Act::Act1Boss => Some(Act::Act2Bushland),
            Act::Act2Bushland => Some(Act::Act3City),
            Act::Act3City => Some(Act::Act4Ocean),
            Act::Act4Ocean => None,
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
            Act::Act1Sanctuary => self.generate_act1(),
            Act::Act1Boss => self.generate_act1_boss(),
            Act::Act2Bushland => self.generate_act2(),
            Act::Act3City => self.generate_act3(),
            Act::Act4Ocean => self.generate_act4(),
        }
    }

    // -------------------------------------------------------------------------
    // ACT 1: HEALESVILLE SANCTUARY (NIGHT COMPOUND)
    // -------------------------------------------------------------------------
    fn generate_act1(&mut self) {
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

        // Exit burrow in far corner
        self.set_cell(21, 21, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 1 CLIMAX: PERIMETER WALL (SEARCHLIGHT MECH BOSS)
    // -------------------------------------------------------------------------
    fn generate_act1_boss(&mut self) {
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

        // Three deep drainage crawl trenches (crawling in these protects Platty from sweeps and stomps!)
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
    // -------------------------------------------------------------------------
    // ACT 2: YARRA RIVER RAPIDS (TEMPLE RUN 5-LANE RUNNER)
    // -------------------------------------------------------------------------
    fn generate_act2(&mut self) {
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

        // River exit flume leading into city storm drains
        self.set_cell(11, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 3: MELBOURNE DOWNTOWN (CITY FROGGER)
    // -------------------------------------------------------------------------
    fn generate_act3(&mut self) {
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

        // Destination: Pier 9 Coastal Railway Terminal at north end
        self.set_cell(12, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 4: COASTAL DUNES & SURF (MARIO 64 3D PLATFORMER)
    // -------------------------------------------------------------------------
    fn generate_act4(&mut self) {
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

        // Pip's Coastal Nesting Burrow atop the high dunes!
        self.set_cell(20, 3, CellType::ExitBurrow);
    }
}
