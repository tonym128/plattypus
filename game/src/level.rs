//! Level maps, tile types, and collision for all 4 acts of Plattypus.

use crate::fixed::Fixed;

pub const LEVEL_W: usize = 140;
pub const LEVEL_H: usize = 24;
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
    BouncyPad = 7,
    BreakableMud = 8,
    WaterCurrentRight = 9,
    WaterCurrentLeft = 10,
    FloatingLog = 11,
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
            player_start_y: Fixed::from_int(16 * 16),
            exit_x: (LEVEL_W as i32 - 6) * TILE_SIZE,
            exit_y: 19 * TILE_SIZE,
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
        t == TileType::Solid || t == TileType::BreakableMud || t == TileType::FloatingLog
    }

    pub fn is_water_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        let t = self.get_tile(tx, ty);
        t == TileType::Water || t == TileType::WaterCurrentRight || t == TileType::WaterCurrentLeft
    }

    pub fn is_bouncy_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        self.get_tile(tx, ty) == TileType::BouncyPad
    }

    pub fn is_breakable_at(&self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        self.get_tile(tx, ty) == TileType::BreakableMud
    }

    pub fn break_tile_at(&mut self, px: i32, py: i32) -> bool {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        if self.get_tile(tx, ty) == TileType::BreakableMud {
            self.set_tile(tx, ty, TileType::Empty);
            true
        } else {
            false
        }
    }

    pub fn get_water_current_at(&self, px: i32, py: i32) -> i8 {
        let tx = px / TILE_SIZE;
        let ty = py / TILE_SIZE;
        match self.get_tile(tx, ty) {
            TileType::WaterCurrentRight => 1,
            TileType::WaterCurrentLeft => -1,
            _ => 0,
        }
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

    /// Act 1: Nocturnal sanctuary pond, tree canopy boardwalks, aviary roof, and water flumes.
    fn build_sanctuary(&mut self) {
        // Bedrock floor
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 23, TileType::Solid);
            self.set_tile(x, 22, TileType::Solid);
            self.set_tile(x, 21, TileType::Solid);
        }

        // Starting sanctuary platform
        for x in 0..16 {
            self.set_tile(x, 19, TileType::Solid);
            self.set_tile(x, 20, TileType::Solid);
        }

        // Bouncy spring lily pad to launch into upper canopy
        self.set_tile(12, 18, TileType::BouncyPad);

        // --- UPPER ROUTE: Tree Canopy & Aviary Walkways (y=6..11) ---
        for x in 14..30 {
            self.set_tile(x, 11, TileType::Platform);
        }
        for x in 34..48 {
            self.set_tile(x, 8, TileType::Platform);
        }
        // Aviary high glass rooftop
        for x in 52..72 {
            self.set_tile(x, 6, TileType::Platform);
        }
        // Huge downhill slide chute from aviary roof down to lagoon
        for x in 74..90 {
            let y = 6 + (x - 74) * 12 / 16;
            self.set_tile(x, y, TileType::SlopeDown);
            for fill in (y + 1)..21 {
                self.set_tile(x, fill, TileType::Solid);
            }
        }

        // --- LOWER ROUTE: Deep Lagoon & Drainage Culverts (y=18..21) ---
        // Deep starting lagoon
        for x in 16..38 {
            for y in 18..22 {
                self.set_tile(x, y, TileType::Water);
            }
        }
        // Floating logs across lagoon
        self.set_tile(22, 17, TileType::FloatingLog);
        self.set_tile(30, 17, TileType::FloatingLog);

        // Mid-lagoon keeper fence with barbed wire
        for y in 14..21 {
            self.set_tile(38, y, TileType::Solid);
        }
        self.set_tile(38, 13, TileType::Hazard); // barbed wire top

        // Subterranean water pipe system
        for x in 42..66 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::WaterCurrentRight);
            }
            self.set_tile(x, 18, TileType::Solid); // pipe ceiling
        }

        // Breakable mud burrow wall hiding secret cavern
        self.set_tile(68, 19, TileType::BreakableMud);
        self.set_tile(68, 20, TileType::BreakableMud);
        self.set_tile(68, 21, TileType::BreakableMud);

        // Deep lagoon pool where both routes converge
        for x in 92..120 {
            for y in 18..22 {
                if y == 21 {
                    self.set_tile(x, y, TileType::WaterCurrentRight);
                } else {
                    self.set_tile(x, y, TileType::Water);
                }
            }
        }

        // Exit sanctuary gate
        for x in 122..LEVEL_W as i32 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        for y in 12..19 {
            self.set_tile(LEVEL_W as i32 - 8, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 6, 19, TileType::Exit);
    }

    /// Act 2: Mountain ridges, steep hills for sliding, billabongs, wombat mounds.
    fn build_bushland(&mut self) {
        // Bedrock floor
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 23, TileType::Solid);
            self.set_tile(x, 22, TileType::Solid);
            self.set_tile(x, 21, TileType::Solid);
        }

        // Starting meadow
        for x in 0..12 {
            self.set_tile(x, 19, TileType::Solid);
            self.set_tile(x, 20, TileType::Solid);
        }

        // Stepping stone ascent to Mountain Ridge
        self.set_tile(14, 17, TileType::Solid);
        self.set_tile(18, 14, TileType::Solid);
        self.set_tile(22, 11, TileType::Solid);
        self.set_tile(26, 8, TileType::Solid);

        // --- UPPER ROUTE: Mountain Ridge & High-Speed Slide Chute ---
        for x in 28..40 {
            for y in 6..21 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        // Massive 30-tile downhill belly slide slope!
        for x in 40..66 {
            let y = 6 + (x - 40) * 12 / 26;
            self.set_tile(x, y, TileType::SlopeDown);
            for fill in (y + 1)..21 {
                self.set_tile(x, fill, TileType::Solid);
            }
        }
        // Super springy tree mushroom launching over the acacia gorge
        self.set_tile(67, 18, TileType::BouncyPad);

        // Prickly acacia bush hazard gorge below
        for x in 70..84 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::Solid);
            }
            self.set_tile(x, 18, TileType::Hazard);
        }

        // High tree canopy platforms across gorge
        for x in 72..86 {
            self.set_tile(x, 10, TileType::Platform);
        }

        // --- LOWER ROUTE: Billabong & Wombat Tunnels ---
        for x in 14..36 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::Water);
            }
        }
        self.set_tile(24, 18, TileType::FloatingLog);

        // Wombat subterranean cavern sealed by breakable mud blocks
        self.set_tile(46, 19, TileType::BreakableMud);
        self.set_tile(46, 20, TileType::BreakableMud);

        // Rushing creek flume
        for x in 90..122 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::WaterCurrentRight);
            }
        }
        self.set_tile(106, 18, TileType::FloatingLog);

        // Culvert pipe leading into Melbourne city
        for x in 124..LEVEL_W as i32 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        for y in 12..19 {
            self.set_tile(LEVEL_W as i32 - 8, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 6, 19, TileType::Exit);
    }

    /// Act 3: Brick buildings, fire escapes, rooftop jumps, tram rails, storm drains.
    fn build_city(&mut self) {
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 23, TileType::Solid);
            self.set_tile(x, 22, TileType::Solid);
            self.set_tile(x, 21, TileType::Solid);
        }

        // Alley start
        for x in 0..14 {
            self.set_tile(x, 19, TileType::Solid);
            self.set_tile(x, 20, TileType::Solid);
        }

        // Steam vent bouncy pad launching Platty up to fire escapes!
        self.set_tile(12, 18, TileType::BouncyPad);

        // --- UPPER ROUTE: Rooftops & Fire Escapes (y=4..12) ---
        // First warehouse building
        for x in 16..32 {
            for y in 13..21 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        // Fire escape ladder platforms
        self.set_tile(34, 15, TileType::Platform);
        self.set_tile(37, 12, TileType::Platform);
        self.set_tile(40, 9, TileType::Platform);

        // High skyscraper tower
        for x in 44..64 {
            for y in 6..21 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        self.set_tile(54, 5, TileType::Hazard); // rooftop AC steam hazard

        // Wall-jump shaft between two skyscrapers!
        for y in 5..16 {
            self.set_tile(66, y, TileType::Solid);
            self.set_tile(72, y, TileType::Solid);
        }

        // Crane beam high platform
        for x in 76..96 {
            self.set_tile(x, 5, TileType::Platform);
        }

        // --- LOWER ROUTE: Street, Tram Lines & Storm Drains (y=18..22) ---
        for x in 64..92 {
            for y in 19..21 {
                self.set_tile(x, y, TileType::Solid);
            }
            if x % 6 == 0 {
                self.set_tile(x, 18, TileType::Hazard); // Electrified tram rails
            }
        }

        // Breakable masonry wall into stormwater drainage main
        self.set_tile(94, 19, TileType::BreakableMud);
        self.set_tile(94, 20, TileType::BreakableMud);

        // Stormwater drain intake with swift flume current
        for x in 96..124 {
            self.set_tile(x, 17, TileType::Solid); // drain ceiling
            for y in 18..22 {
                if y == 21 {
                    self.set_tile(x, y, TileType::WaterCurrentRight);
                } else {
                    self.set_tile(x, y, TileType::Water);
                }
            }
        }

        // Exit stormwater pipe outfall
        for x in 126..LEVEL_W as i32 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        for y in 12..19 {
            self.set_tile(LEVEL_W as i32 - 8, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 6, 19, TileType::Exit);
    }

    /// Act 4: Coastal cliffs, sandy beaches, tidal rockpools, breaking waves, family burrow!
    fn build_ocean(&mut self) {
        for x in 0..LEVEL_W as i32 {
            self.set_tile(x, 23, TileType::Solid);
            self.set_tile(x, 22, TileType::Solid);
            self.set_tile(x, 21, TileType::Solid);
        }

        // Starting beach
        for x in 0..14 {
            self.set_tile(x, 19, TileType::Solid);
            self.set_tile(x, 20, TileType::Solid);
        }

        // Sea-kelp springy pad
        self.set_tile(12, 18, TileType::BouncyPad);

        // --- UPPER ROUTE: Sandstone Cliffs & Sandy Slide ---
        for x in 16..38 {
            for y in 11..21 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        // High coastal sandstone headland
        for x in 40..58 {
            for y in 6..21 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        // Mega sand slide descending to the estuary mouth
        for x in 58..84 {
            let y = 6 + (x - 58) * 12 / 26;
            self.set_tile(x, y, TileType::SlopeDown);
            for fill in (y + 1)..21 {
                self.set_tile(x, fill, TileType::Solid);
            }
        }

        // --- LOWER ROUTE: Tidal Rock Pools & Submerged Marine Grottos ---
        for x in 16..42 {
            for y in 18..22 {
                self.set_tile(x, y, TileType::Water);
            }
        }
        self.set_tile(24, 17, TileType::FloatingLog);
        self.set_tile(32, 17, TileType::FloatingLog);

        // Underwater cave system with fast tidal current
        for x in 44..72 {
            for y in 19..22 {
                self.set_tile(x, y, TileType::WaterCurrentRight);
            }
        }

        // Breakable sandstone barrier guarding family secret stash
        self.set_tile(88, 19, TileType::BreakableMud);
        self.set_tile(88, 20, TileType::BreakableMud);

        // The Native Habitat River Estuary
        for x in 90..124 {
            for y in 18..22 {
                self.set_tile(x, y, TileType::Water);
            }
        }
        self.set_tile(104, 17, TileType::FloatingLog);

        // The Family Burrow!
        for x in 126..LEVEL_W as i32 {
            for y in 18..22 {
                self.set_tile(x, y, TileType::Solid);
            }
        }
        for y in 11..19 {
            self.set_tile(LEVEL_W as i32 - 8, y, TileType::Solid);
        }
        self.set_tile(LEVEL_W as i32 - 6, 19, TileType::Exit);
    }
}
