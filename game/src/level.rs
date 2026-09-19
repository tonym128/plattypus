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
    Act2Bushland,
    Act3City,
    Act4Ocean,
}

impl Act {
    pub fn title(&self) -> &'static str {
        match self {
            Act::Act1Sanctuary => "ACT 1: HEALESVILLE COMPOUND (NIGHT)",
            Act::Act2Bushland => "ACT 2: YARRA VALLEY BUSHLAND",
            Act::Act3City => "ACT 3: MELBOURNE CARGO DOCKS",
            Act::Act4Ocean => "ACT 4: THE NATIVE COAST & ESTUARY",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Act::Act1Sanctuary => "Execute tactical stealth escape past sentry searchlights!",
            Act::Act2Bushland => "Crawl through scrub and ride water flumes under drone patrol!",
            Act::Act3City => "Navigate shipping container maze and avoid surveillance lasers!",
            Act::Act4Ocean => "Submerge beneath seawall searchlights and reach the family burrow!",
        }
    }

    pub fn next(&self) -> Option<Act> {
        match self {
            Act::Act1Sanctuary => Some(Act::Act2Bushland),
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
    // ACT 2: YARRA VALLEY BUSHLAND (WILDERNESS FLUMES)
    // -------------------------------------------------------------------------
    fn generate_act2(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 21 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 2 * TILE_SZ + 32;

        // River flume winding across the bushland
        for x in 1..22 {
            self.set_cell(x, 11, CellType::WaterCurrent);
            self.set_cell(x, 12, CellType::Water);
        }

        // Red rock canyon ridges
        for x in 3..10 {
            self.set_cell(x, 6, CellType::Wall);
            self.set_cell(x, 17, CellType::Wall);
        }
        for x in 14..21 {
            self.set_cell(x, 6, CellType::Wall);
            self.set_cell(x, 17, CellType::Wall);
        }

        // Timber suspension bridge / crossing
        self.set_cell(11, 11, CellType::Floor);
        self.set_cell(11, 12, CellType::Floor);

        // Abundant Australian scrub & tall grass for crawl camouflage
        for x in 2..22 {
            if x % 3 == 0 {
                self.set_cell(x, 4, CellType::TallGrass);
                self.set_cell(x, 9, CellType::TallGrass);
                self.set_cell(x, 14, CellType::TallGrass);
                self.set_cell(x, 19, CellType::TallGrass);
            }
        }

        // Termite mounds and boulders (represented as crates/boulders)
        let boulders = [
            (5, 8), (6, 8), (17, 8), (18, 8),
            (4, 15), (7, 15), (16, 15), (19, 15),
            (10, 3), (12, 3), (14, 20),
        ];
        for (bx, bz) in boulders {
            self.set_cell(bx, bz, CellType::Crate);
        }

        self.set_cell(21, 2, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 3: MELBOURNE CARGO DOCKS (SHIPPING YARD)
    // -------------------------------------------------------------------------
    fn generate_act3(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 12 * TILE_SZ + 32;
        self.exit_x = 21 * TILE_SZ + 32;
        self.exit_z = 12 * TILE_SZ + 32;

        // Giant freight shipping containers (Container blocks)
        let containers = [
            (5, 3), (6, 3), (7, 3), (8, 3),
            (5, 6), (6, 6), (7, 6), (8, 6),
            (5, 17), (6, 17), (7, 17), (8, 17),
            (5, 20), (6, 20), (7, 20), (8, 20),

            (15, 3), (16, 3), (17, 3), (18, 3),
            (15, 6), (16, 6), (17, 6), (18, 6),
            (15, 17), (16, 17), (17, 17), (18, 17),
            (15, 20), (16, 20), (17, 20), (18, 20),
        ];
        for (cx, cz) in containers {
            self.set_cell(cx, cz, CellType::Container);
        }

        // Central railway tracks & security gate
        for z in 2..22 {
            self.set_cell(11, z, CellType::Wall);
        }
        self.set_cell(11, 8, CellType::LaserTripwire);
        self.set_cell(11, 12, CellType::AirDuct); // Ventilation pipe under tracks!
        self.set_cell(11, 16, CellType::LaserTripwire);

        // Forklift pallets and cargo crates
        let crates = [
            (3, 8), (3, 16),
            (9, 9), (9, 15),
            (13, 8), (13, 16),
            (19, 9), (19, 15),
        ];
        for (cx, cz) in crates {
            self.set_cell(cx, cz, CellType::Crate);
        }

        self.set_cell(21, 12, CellType::ExitBurrow);
    }

    // -------------------------------------------------------------------------
    // ACT 4: THE NATIVE COAST & ESTUARY (FINAL INFILTRATION)
    // -------------------------------------------------------------------------
    fn generate_act4(&mut self) {
        self.player_start_x = 2 * TILE_SZ + 32;
        self.player_start_z = 2 * TILE_SZ + 32;
        self.exit_x = 20 * TILE_SZ + 32;
        self.exit_z = 20 * TILE_SZ + 32;

        // Seawall fortifications
        for x in 1..18 {
            self.set_cell(x, 8, CellType::Wall);
        }
        self.set_cell(8, 8, CellType::AirDuct); // Submerged drain conduit

        // Deep ocean trench & surf
        for z in 14..22 {
            for x in 1..22 {
                self.set_cell(x, z, CellType::Water);
            }
        }

        // Seawall breakwater pillars
        for x in (3..20).step_by(4) {
            self.set_cell(x, 13, CellType::Container);
        }

        // Coastal dune grass & pier pilings
        let grass = [
            (3, 4), (4, 4), (5, 5), (6, 5),
            (12, 4), (13, 4), (14, 5),
            (8, 10), (9, 10), (14, 11),
        ];
        for (gx, gz) in grass {
            self.set_cell(gx, gz, CellType::TallGrass);
        }

        // The Family Burrow entrance right on the sandy shore!
        self.set_cell(20, 20, CellType::ExitBurrow);
    }
}
