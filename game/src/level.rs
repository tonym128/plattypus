//! Level maps, tile types, and collision for all 4 acts of Plattypus.

use crate::fixed::Fixed;

pub const LEVEL_W: usize = 120;
pub const LEVEL_H: usize = 15;
pub const TILE_SIZE: i32 = 16;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TileType {
    Empty = 0,
    Solid = 1,
    SlopeDown = 2,
    Water = 3,
    Hazard = 4,
    Platform = 5,
    Exit = 6,
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
            Act::Act1Sanctuary => "ACT 1: HEALESVILLE SANCTUARY (NIGHT)",
            Act::Act2Bushland => "ACT 2: YARRA VALLEY BUSHLAND",
            Act::Act3City => "ACT 3: MELBOURNE TRANSIT & ROOFS",
            Act::Act4Ocean => "ACT 4: THE NATIVE COAST & ESTUARY",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Act::Act1Sanctuary => "Sneak past the night watch to begin the trek!",
            Act::Act2Bushland => "Belly-slide the creek banks and bush roads!",
            Act::Act3City => "Navigate city rooftops and storm drains!",
            Act::Act4Ocean => "Reach the family burrow to meet your new sibling!",
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
    pub tiles: [TileType; LEVEL_W * LEVEL_H],
    pub player_start_x: Fixed,
    pub player_start_y: Fixed,
    pub exit_x: i32,
    pub exit_y: i32,
}

impl Level {
    pub fn new(act: Act) -> Self {
        let mut level = Self {
            act,
            tiles: [TileType::Empty; LEVEL_W * LEVEL_H],
            player_start_x: Fixed::from_int(32),
            player_start_y: Fixed::from_int(160),
            exit_x: (LEVEL_W as i32 - 4) * TILE_SIZE,
            exit_y: 12 * TILE_SIZE,
        };
        level.build_act();
        level
    }

    #[inline(always)]
    pub fn get_tile(&self, tx: i32, ty: i32) -> TileType {
        if tx < 0 || tx >= LEVEL_W as i32 || ty < 0 || ty >= LEVEL_H as i32 {
            return TileType::Solid;
        }
        self.tiles[(ty as usize) * LEVEL_W + (tx as usize)]
    }

    #[inline(always)]
    pub fn set_tile(&mut self, tx: i32, ty: i32, tile: TileType) {
        if tx >= 0 && tx < LEVEL_W as i32 && ty >= 0 && ty < LEVEL_H as i32 {
            self.tiles[(ty as usize) * LEVEL_W + (tx as usize)] = tile;
        }
    }

    pub fn is_solid_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        let t = self.get_tile(tx, ty);
        t == TileType::Solid
    }

    pub fn is_water_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        self.get_tile(tx, ty) == TileType::Water
    }

    pub fn is_hazard_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        self.get_tile(tx, ty) == TileType::Hazard
    }

    pub fn is_exit_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        self.get_tile(tx, ty) == TileType::Exit
    }

    fn build_act(&mut self) {
        match self.act {
            Act::Act1Sanctuary => self.build_sanctuary(),
            Act::Act2Bushland => self.build_bushland(),
            Act::Act3City => self.build_city(),
            Act::Act4Ocean => self.build_ocean(),
        }
    }

    /// Act 1: Nocturnal sanctuary pond, wooden boardwalks, fences, water flume exit.
    fn build_sanctuary(&mut self) {
        // Base ground and pond
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 14, TileType::Solid);
            self.set_tile(x, 13, TileType::Solid);
        }

        // Starting pond
        for x in 4..16 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
        }

        // Boardwalk platforms
        for x in 18..28 {
            self.set_tile(x, 10, TileType::Platform);
        }

        // Sanctuary keeper fence
        self.set_tile(32, 12, TileType::Solid);
        self.set_tile(32, 11, TileType::Solid);
        self.set_tile(32, 10, TileType::Hazard); // barbed top

        // Middle pond with floating logs
        for x in 36..52 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
        }
        self.set_tile(40, 11, TileType::Platform);
        self.set_tile(46, 11, TileType::Platform);

        // Aviary structure
        for x in 56..70 {
            self.set_tile(x, 9, TileType::Platform);
        }
        for y in 9..13 {
            self.set_tile(70, y, TileType::Solid);
        }

        // Water drain flume slide
        for x in 76..92 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::SlopeDown);
        }

        // Exit gate
        for y in 8..13 {
            self.set_tile(LEVEL_W as i32 - 6, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 4, 12, TileType::Exit);
    }

    /// Act 2: Winding bush tracks, steep hills for sliding, billabongs, wombat mounds.
    fn build_bushland(&mut self) {
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 14, TileType::Solid);
        }

        // Big downhill slope for high-speed belly slide!
        for x in 10..22 {
            let y = 8 + (x - 10) / 3;
            self.set_tile(x, y, TileType::SlopeDown);
            for fill in (y + 1)..14 {
                self.set_tile(x, fill, TileType::Solid);
            }
        }

        // Deep billabong swimming segment
        for x in 24..44 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
            self.set_tile(x, 11, TileType::Water);
        }

        // Stepping stone platforms
        self.set_tile(48, 12, TileType::Solid);
        self.set_tile(52, 11, TileType::Solid);
        self.set_tile(56, 10, TileType::Solid);

        // Prickly acacia bush hazards
        self.set_tile(60, 13, TileType::Hazard);
        self.set_tile(61, 13, TileType::Hazard);

        // Wombat hill
        for x in 66..78 {
            self.set_tile(x, 11, TileType::Solid);
            self.set_tile(x, 12, TileType::Solid);
            self.set_tile(x, 13, TileType::Solid);
        }

        // Creek crossing
        for x in 82..98 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
        }

        // Culvert pipe leading to city
        for y in 9..14 {
            self.set_tile(LEVEL_W as i32 - 8, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 4, 13, TileType::Exit);
    }

    /// Act 3: Brick buildings, fire escapes, rooftop jumps, tram rails, storm drains.
    fn build_city(&mut self) {
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 14, TileType::Solid);
        }

        // First building
        for x in 8..20 {
            for y in 9..14 {
                self.set_tile(x, y, TileType::Solid);
            }
        }

        // Fire escape platforms
        self.set_tile(22, 11, TileType::Platform);
        self.set_tile(25, 9, TileType::Platform);
        self.set_tile(28, 7, TileType::Platform);

        // Second taller building
        for x in 30..44 {
            for y in 6..14 {
                self.set_tile(x, y, TileType::Solid);
            }
        }

        // Steam vent hazard on rooftop
        self.set_tile(37, 5, TileType::Hazard);

        // Tram line gap below
        for x in 48..64 {
            self.set_tile(x, 13, TileType::Solid);
            // Electrified tram rail
            if x % 4 == 0 {
                self.set_tile(x, 12, TileType::Hazard);
            }
        }

        // Stormwater drain intake
        for x in 68..88 {
            for y in 10..14 {
                self.set_tile(x, y, TileType::Water);
            }
        }

        // Underground pipe slope
        for x in 90..102 {
            self.set_tile(x, 12, TileType::SlopeDown);
            self.set_tile(x, 13, TileType::Solid);
        }

        self.set_tile(LEVEL_W as i32 - 4, 12, TileType::Exit);
    }

    /// Act 4: Coastal cliffs, sandy beaches, tidal rockpools, breaking waves, family burrow!
    fn build_ocean(&mut self) {
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 14, TileType::Solid);
        }

        // Sand dunes
        for x in 8..18 {
            self.set_tile(x, 12, TileType::Solid);
            self.set_tile(x, 13, TileType::Solid);
        }

        // Tidal estuary swimming
        for x in 22..46 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
            self.set_tile(x, 11, TileType::Water);
        }

        // Rock pool pillars
        for x in 50..64 {
            if x % 3 != 0 {
                self.set_tile(x, 11, TileType::Solid);
                self.set_tile(x, 12, TileType::Solid);
                self.set_tile(x, 13, TileType::Solid);
            } else {
                self.set_tile(x, 13, TileType::Water);
            }
        }

        // Coastal sandstone cliff
        for x in 68..80 {
            for y in 8..14 {
                self.set_tile(x, y, TileType::Solid);
            }
        }

        // Big sandy slide down to the river mouth
        for x in 82..94 {
            let y = 8 + (x - 82) / 3;
            self.set_tile(x, y, TileType::SlopeDown);
            for fill in (y + 1)..14 {
                self.set_tile(x, fill, TileType::Solid);
            }
        }

        // The Native Habitat River Estuary
        for x in 96..112 {
            self.set_tile(x, 13, TileType::Water);
            self.set_tile(x, 12, TileType::Water);
        }

        // The Family Burrow
        for y in 10..14 {
            self.set_tile(LEVEL_W as i32 - 5, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 4, 12, TileType::Exit);
    }
}
