//! Procedural VRAM texture atlas generator & Gouraud shading engine for Plattypus MGS.
//! Generates 4bpp tactile retro textures and 15-bit BGR555 CLUT palettes,
//! loaded directly into PlayStation VRAM on boot.

use psx_vram::{upload_16bpp, upload_clut, Clut, Color555, TexDepth, Tpage, VramRect};

/// Atlas resides at VRAM X=384, Y=0 (outside framebuffers and BIOS font).
pub const ATLAS_TPAGE: Tpage = Tpage::new(384, 0, TexDepth::Bit4);

/// CLUTs reside at VRAM X=384, Y=256.
pub const ATLAS_CLUT_X: u16 = 384;
pub const ATLAS_CLUT_Y: u16 = 256;

// CLUT Bank indices
pub const BANK_MILITARY: u8 = 0;
pub const BANK_PLATTY: u8 = 1;
pub const BANK_RIVER: u8 = 2;
pub const BANK_CITY: u8 = 3;
pub const BANK_BEACH: u8 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TextureId {
    Crate,
    MetalGrate,
    ConcreteWall,
    CautionStripe,
    PlattyFur,
    PlattyBill,
    PlattyBandana,
    SentryCamo,
    RiverWater,
    RiverLog,
    GumLeaves,
    CityAsphalt,
    CityBrick,
    CarGrill,
    BeachSand,
    Parasol,
    RockCliff,
}

impl TextureId {
    /// Returns (clut_bank, [(u, v); 4]) for Top-Left, Top-Right, Bottom-Left, Bottom-Right.
    pub fn uv_and_bank(&self) -> (u8, [(u8, u8); 4]) {
        let (bank, (u, v)) = match self {
            TextureId::Crate => (BANK_MILITARY, (0, 0)),
            TextureId::MetalGrate => (BANK_MILITARY, (16, 0)),
            TextureId::ConcreteWall => (BANK_MILITARY, (32, 0)),
            TextureId::CautionStripe => (BANK_MILITARY, (48, 0)),

            TextureId::PlattyFur => (BANK_PLATTY, (0, 16)),
            TextureId::PlattyBill => (BANK_PLATTY, (16, 16)),
            TextureId::PlattyBandana => (BANK_PLATTY, (32, 16)),
            TextureId::SentryCamo => (BANK_MILITARY, (48, 16)),

            TextureId::RiverWater => (BANK_RIVER, (0, 32)),
            TextureId::RiverLog => (BANK_RIVER, (16, 32)),
            TextureId::GumLeaves => (BANK_RIVER, (32, 32)),

            TextureId::CityAsphalt => (BANK_CITY, (0, 48)),
            TextureId::CityBrick => (BANK_CITY, (16, 48)),
            TextureId::CarGrill => (BANK_CITY, (32, 48)),

            TextureId::BeachSand => (BANK_BEACH, (0, 64)),
            TextureId::Parasol => (BANK_BEACH, (16, 64)),
            TextureId::RockCliff => (BANK_BEACH, (32, 64)),
        };

        (bank, [(u, v), (u + 15, v), (u, v + 15), (u + 15, v + 15)])
    }
}

pub struct TextureAtlasManager {
    pub tpage_word: u16,
    pub clut_words: [u16; 5],
}

impl TextureAtlasManager {
    pub fn new() -> Self {
        let tpage_word = ATLAS_TPAGE.uv_tpage_word(0);
        let mut clut_words = [0u16; 5];
        for b in 0..5 {
            let clut = Clut::new(ATLAS_CLUT_X + (b as u16 * 16), ATLAS_CLUT_Y);
            clut_words[b as usize] = clut.uv_clut_word();
        }

        Self {
            tpage_word,
            clut_words,
        }
    }

    /// Upload all procedural texture patterns and 15-bit CLUT banks to VRAM.
    pub fn init_and_upload(&self) {
        self.upload_cluts();
        self.upload_textures();
    }

    fn upload_cluts(&self) {
        // Bank 0: Tactical Military (Crates, metal grates, concrete, hazard stripes)
        let pal_military = [
            Color555::rgb8(20, 22, 25),    // 0: Deep shadow
            Color555::rgb8(45, 50, 42),    // 1: Dark olive
            Color555::rgb8(75, 82, 65),    // 2: Olive drab
            Color555::rgb8(110, 118, 95),  // 3: Light olive
            Color555::rgb8(40, 44, 48),    // 4: Dark steel
            Color555::rgb8(70, 78, 85),    // 5: Gunmetal
            Color555::rgb8(120, 130, 140), // 6: Polished steel
            Color555::rgb8(180, 190, 200), // 7: Steel highlight
            Color555::rgb8(60, 60, 60),    // 8: Dark concrete
            Color555::rgb8(95, 95, 95),    // 9: Mid concrete
            Color555::rgb8(140, 140, 140), // 10: Light concrete
            Color555::rgb8(180, 180, 175), // 11: Concrete highlight
            Color555::rgb8(210, 170, 30),  // 12: Hazard yellow
            Color555::rgb8(245, 210, 50),  // 13: Hazard bright yellow
            Color555::rgb8(220, 50, 40),   // 14: Warning red
            Color555::rgb8(240, 240, 240), // 15: Stencil white
        ];
        upload_clut(Clut::new(ATLAS_CLUT_X, ATLAS_CLUT_Y), &pal_military);

        // Bank 1: Platty (Fur, duckbill, tail, bandana)
        let pal_platty = [
            Color555::rgb8(15, 12, 10),    // 0: Fur outline/shadow
            Color555::rgb8(55, 35, 20),    // 1: Dark beaver fur
            Color555::rgb8(95, 60, 35),    // 2: Mid fur
            Color555::rgb8(140, 90, 50),   // 3: Golden fur highlight
            Color555::rgb8(180, 125, 75),  // 4: Sunlit fur tip
            Color555::rgb8(25, 30, 35),    // 5: Bill deep charcoal
            Color555::rgb8(45, 52, 58),    // 6: Bill leathery grey
            Color555::rgb8(75, 85, 95),    // 7: Bill sensory ridge
            Color555::rgb8(110, 125, 135), // 8: Bill specular sheen
            Color555::rgb8(80, 15, 20),    // 9: Bandana dark crimson
            Color555::rgb8(180, 30, 35),   // 10: Bandana red
            Color555::rgb8(240, 60, 65),   // 11: Bandana bright crimson
            Color555::rgb8(255, 220, 120), // 12: Spur venom gold
            Color555::rgb8(20, 80, 95),    // 13: Webbed foot teal-slate
            Color555::rgb8(10, 10, 12),    // 14: Eye pupil
            Color555::rgb8(250, 250, 250), // 15: Eye catchlight
        ];
        upload_clut(Clut::new(ATLAS_CLUT_X + 16, ATLAS_CLUT_Y), &pal_platty);

        // Bank 2: River & Bushland (Eucalyptus, rapids foam, muddy bank)
        let pal_river = [
            Color555::rgb8(10, 20, 25),    // 0: Deep river trench
            Color555::rgb8(20, 50, 65),    // 1: Dark Yarra water
            Color555::rgb8(35, 90, 115),   // 2: Rushing river blue
            Color555::rgb8(70, 150, 175),  // 3: River ripple cyan
            Color555::rgb8(160, 220, 235), // 4: River rapids foam
            Color555::rgb8(240, 250, 255), // 5: Whitecap spray
            Color555::rgb8(30, 45, 25),    // 6: Deep bush canopy
            Color555::rgb8(55, 85, 45),    // 7: Eucalyptus green
            Color555::rgb8(95, 135, 75),   // 8: Gum leaf green
            Color555::rgb8(150, 185, 115), // 9: Sunlit leaf tip
            Color555::rgb8(45, 30, 20),    // 10: Bark dark trench
            Color555::rgb8(85, 60, 40),    // 11: Weathered trunk wood
            Color555::rgb8(130, 100, 70),  // 12: Tree bark tan
            Color555::rgb8(180, 150, 110), // 13: Dry bark highlight
            Color555::rgb8(200, 140, 40),  // 14: Tiger snake gold
            Color555::rgb8(220, 60, 30),   // 15: Huntsman spider alert
        ];
        upload_clut(Clut::new(ATLAS_CLUT_X + 32, ATLAS_CLUT_Y), &pal_river);

        // Bank 3: City & Asphalt (Road, bricks, taxi yellow, metallic grilles)
        let pal_city = [
            Color555::rgb8(12, 14, 18),    // 0: Night shadow
            Color555::rgb8(30, 32, 38),    // 1: Wet asphalt
            Color555::rgb8(50, 54, 62),    // 2: Mid asphalt
            Color555::rgb8(75, 80, 90),    // 3: Light road surface
            Color555::rgb8(230, 230, 240), // 4: White lane divider
            Color555::rgb8(60, 20, 20),    // 5: Dark brick mortar
            Color555::rgb8(120, 45, 40),   // 6: Red clay brick
            Color555::rgb8(170, 70, 60),   // 7: Bright brick face
            Color555::rgb8(200, 150, 20),  // 8: Melbourne taxi yellow
            Color555::rgb8(250, 200, 40),  // 9: Bright yellow taxi hood
            Color555::rgb8(30, 70, 140),   // 10: Melbourne blue tram stripe
            Color555::rgb8(220, 220, 240), // 11: Chrome car bumper
            Color555::rgb8(240, 230, 120), // 12: Headlight glow
            Color555::rgb8(210, 30, 30),   // 13: Tail light red
            Color555::rgb8(30, 90, 80),    // 14: Dark glass tint
            Color555::rgb8(120, 210, 200), // 15: Neon cyan reflection
        ];
        upload_clut(Clut::new(ATLAS_CLUT_X + 48, ATLAS_CLUT_Y), &pal_city);

        // Bank 4: Coastal Beach (Sand, parasol, coastal rock, crab)
        let pal_beach = [
            Color555::rgb8(25, 20, 15),    // 0: Cave shadow
            Color555::rgb8(130, 105, 60),  // 1: Damp sand
            Color555::rgb8(190, 160, 100), // 2: Golden beach sand
            Color555::rgb8(230, 205, 145), // 3: Sun-bleached dune
            Color555::rgb8(255, 240, 200), // 4: Dune crest highlight
            Color555::rgb8(60, 50, 40),    // 5: Dark sandstone
            Color555::rgb8(110, 90, 70),   // 6: Cliff rock strata
            Color555::rgb8(160, 135, 110), // 7: Weathered cliff face
            Color555::rgb8(200, 50, 60),   // 8: Parasol coral stripe
            Color555::rgb8(240, 100, 110), // 9: Bright coral parasol
            Color555::rgb8(30, 140, 150),  // 10: Parasol turquoise stripe
            Color555::rgb8(80, 210, 220),  // 11: Bright turquoise parasol
            Color555::rgb8(240, 80, 35),   // 12: Ghost crab shell
            Color555::rgb8(255, 140, 70),  // 13: Bright crab claw
            Color555::rgb8(180, 230, 245), // 14: Ocean surf wave
            Color555::rgb8(250, 255, 255), // 15: Sea foam white
        ];
        upload_clut(Clut::new(ATLAS_CLUT_X + 64, ATLAS_CLUT_Y), &pal_beach);
    }

    fn upload_textures(&self) {
        // Upload 64x80 4bpp pixel block (16 VRAM halfwords wide x 80 high = 1,280 halfwords).
        // In 4bpp, each halfword holds 4 pixels (bits 0..3, 4..7, 8..11, 12..15).
        let mut vram_buf = [0u16; 16 * 80];

        // 1. Crate (Tile 0,0: 16x16)
        // Outer beveled border, cross-brace diagonal, corner rivets
        for y in 0..16 {
            for x in 0..16 {
                let p = if x == 0 || x == 15 || y == 0 || y == 15 {
                    1u8 // Dark frame
                } else if x == 1 || y == 1 {
                    3 // Light bevel
                } else if x == 14 || y == 14 {
                    1 // Dark bevel
                } else if (x == 2 || x == 13) && (y == 2 || y == 13) {
                    15 // Corner rivet
                } else if x == y || x == (15 - y) {
                    3 // Diagonal cross brace
                } else {
                    2 // Mid olive wood panel
                };
                set_4bpp_pixel(&mut vram_buf, 0 + x, 0 + y, p);
            }
        }

        // 2. Metal Grate (Tile 16,0: 16x16)
        // Perforated industrial drainage grill
        for y in 0..16 {
            for x in 0..16 {
                let is_hole = (x % 4 == 1 || x % 4 == 2) && (y % 4 == 1 || y % 4 == 2);
                let p = if is_hole {
                    0u8 // Deep shadow hole
                } else if x % 4 == 0 || y % 4 == 0 {
                    7 // Steel highlight rim
                } else {
                    5 // Gunmetal bar
                };
                set_4bpp_pixel(&mut vram_buf, 16 + x, 0 + y, p);
            }
        }

        // 3. Concrete Wall (Tile 32,0: 16x16)
        // Stippled weathered concrete with bolt line
        for y in 0..16 {
            for x in 0..16 {
                let hash = ((x * 17 + y * 31) ^ (x * y)) & 3;
                let p = if y == 4 || y == 12 {
                    if x % 4 == 2 {
                        11
                    } else {
                        8
                    } // Bolt lines
                } else {
                    8 + (hash as u8) // Texture grain (8, 9, 10, 11)
                };
                set_4bpp_pixel(&mut vram_buf, 32 + x, 0 + y, p);
            }
        }

        // 4. Caution Hazard Stripe (Tile 48,0: 16x16)
        // Diagonal 45-degree yellow and dark hazard stripes
        for y in 0..16 {
            for x in 0..16 {
                let stripe = (x + y) % 8 < 4;
                let p = if stripe { 12 } else { 0 };
                set_4bpp_pixel(&mut vram_buf, 48 + x, 0 + y, p);
            }
        }

        // 5. Platty Fur (Tile 0,16: 16x16)
        // Dense pelt with subtle fur strand highlights
        for y in 0..16 {
            for x in 0..16 {
                let strand = ((x * 13 + y * 7) % 5) as u8;
                let p = match strand {
                    0 => 1,
                    1 => 2,
                    2 => 3,
                    3 => 4,
                    _ => 2,
                };
                set_4bpp_pixel(&mut vram_buf, 0 + x, 16 + y, p);
            }
        }

        // 6. Platty Bill (Tile 16,16: 16x16)
        // Leathery bill with electro-receptor pit dots
        for y in 0..16 {
            for x in 0..16 {
                let is_pit = (x % 3 == 1) && (y % 3 == 1);
                let p = if is_pit {
                    5 // Sensory pit dot
                } else if y == 0 || y == 15 || x == 0 || x == 15 {
                    7 // Bill edge ridge
                } else {
                    6 // Leathery slate
                };
                set_4bpp_pixel(&mut vram_buf, 16 + x, 16 + y, p);
            }
        }

        // 7. Bandana (Tile 32,16: 16x16)
        // Tactical crimson fabric folds
        for y in 0..16 {
            for x in 0..16 {
                let fold = (y % 4) as u8;
                let p = match fold {
                    0 => 9,
                    1 => 10,
                    2 => 11,
                    _ => 10,
                };
                set_4bpp_pixel(&mut vram_buf, 32 + x, 16 + y, p);
            }
        }

        // 8. Sentry Camo (Tile 48,16: 16x16)
        // Shadow urban camo blotches
        for y in 0..16 {
            for x in 0..16 {
                let blotch = ((x ^ y) + (x * 3)) % 7;
                let p = if blotch < 2 {
                    4 // Dark steel
                } else if blotch < 5 {
                    5 // Gunmetal
                } else {
                    1 // Olive drab
                };
                set_4bpp_pixel(&mut vram_buf, 48 + x, 16 + y, p);
            }
        }

        // 9. River Water / Foam (Tile 0,32: 16x16)
        // Rapids foam swirling waves
        for y in 0..16 {
            for x in 0..16 {
                let wave = ((x + y * 2) % 6) as u8;
                let p = match wave {
                    0 => 1,
                    1 => 2,
                    2 => 3,
                    3 => 4,
                    4 => 5,
                    _ => 2,
                };
                set_4bpp_pixel(&mut vram_buf, 0 + x, 32 + y, p);
            }
        }

        // 10. River Log (Tile 16,32: 16x16)
        // Wood grain with dark fissures
        for y in 0..16 {
            for x in 0..16 {
                let fissure = (x % 5 == 0) || (y % 7 == 2);
                let p = if fissure {
                    10
                } else if x % 2 == 0 {
                    11
                } else {
                    12
                };
                set_4bpp_pixel(&mut vram_buf, 16 + x, 32 + y, p);
            }
        }

        // 11. Gum Leaves (Tile 32,32: 16x16)
        // Eucalyptus canopy foliage
        for y in 0..16 {
            for x in 0..16 {
                let leaf = ((x * 11 + y * 19) % 6) as u8;
                let p = match leaf {
                    0 => 6,
                    1 => 7,
                    2 => 8,
                    3 => 9,
                    _ => 7,
                };
                set_4bpp_pixel(&mut vram_buf, 32 + x, 32 + y, p);
            }
        }

        // 12. City Asphalt (Tile 0,48: 16x16)
        // Road aggregate with lane paint stripe
        for y in 0..16 {
            for x in 0..16 {
                let is_line = (x >= 7 && x <= 8) && (y < 12);
                let p = if is_line {
                    4 // White road line
                } else {
                    1 + (((x * 23 + y * 37) & 3) as u8).min(2) // Asphalt aggregate
                };
                set_4bpp_pixel(&mut vram_buf, 0 + x, 48 + y, p);
            }
        }

        // 13. City Brick (Tile 16,48: 16x16)
        // Melbourne alleyway red brick pattern
        for y in 0..16 {
            for x in 0..16 {
                let row = y / 4;
                let offset = if (row & 1) != 0 { 4 } else { 0 };
                let is_mortar = (y % 4 == 0) || ((x + offset) % 8 == 0);
                let p = if is_mortar {
                    5 // Dark mortar
                } else if y % 4 == 1 {
                    7 // Top brick highlight
                } else {
                    6 // Brick red
                };
                set_4bpp_pixel(&mut vram_buf, 16 + x, 48 + y, p);
            }
        }

        // 14. Car Grill (Tile 32,48: 16x16)
        // Automotive horizontal chrome slats and headlights
        for y in 0..16 {
            for x in 0..16 {
                let is_headlight = (x < 3 || x > 12) && (y >= 4 && y <= 8);
                let p = if is_headlight {
                    12 // Glowing headlight
                } else if y % 2 == 0 {
                    11 // Chrome slat
                } else {
                    0 // Black recessed intake
                };
                set_4bpp_pixel(&mut vram_buf, 32 + x, 48 + y, p);
            }
        }

        // 15. Beach Sand (Tile 0,64: 16x16)
        // Rippled wind-swept golden dune
        for y in 0..16 {
            for x in 0..16 {
                let ripple = ((x + (y / 2)) % 4) as u8;
                let p = 1 + ripple;
                set_4bpp_pixel(&mut vram_buf, 0 + x, 64 + y, p);
            }
        }

        // 16. Parasol (Tile 16,64: 16x16)
        // Alternating coral & turquoise umbrella panels
        for y in 0..16 {
            for x in 0..16 {
                let wedge = (x / 4) & 1;
                let p = if wedge == 0 {
                    8 + (y % 2) as u8 // Coral stripe
                } else {
                    10 + (y % 2) as u8 // Turquoise stripe
                };
                set_4bpp_pixel(&mut vram_buf, 16 + x, 64 + y, p);
            }
        }

        // 17. Rock Cliff (Tile 32,64: 16x16)
        // Layered coastal sandstone strata
        for y in 0..16 {
            for x in 0..16 {
                let strata = (y / 3) % 3;
                let p = match strata {
                    0 => 5,
                    1 => 6,
                    _ => 7,
                };
                set_4bpp_pixel(&mut vram_buf, 32 + x, 64 + y, p);
            }
        }

        // Upload 64x80 region into VRAM at (ATLAS_TPAGE.x, ATLAS_TPAGE.y).
        // Width in VRAM halfwords = 64 / 4 = 16.
        let rect = VramRect::new(ATLAS_TPAGE.x(), ATLAS_TPAGE.y(), 16, 80);
        upload_16bpp(rect, &vram_buf);
    }
}

/// Set a single 4-bit pixel (0..15) at (x, y) in a 64-pixel-wide 4bpp buffer.
#[inline]
fn set_4bpp_pixel(buf: &mut [u16], x: usize, y: usize, color_index: u8) {
    let halfword_x = x / 4;
    let nibble = x % 4;
    let idx = y * 16 + halfword_x;
    if idx < buf.len() {
        let shift = nibble * 4;
        let mask = !(0x0F << shift);
        buf[idx] = (buf[idx] & mask) | (((color_index & 0x0F) as u16) << shift);
    }
}

/// Directional Gouraud vertex lighting calculator.
/// Computes per-vertex tints (TL, TR, BL, BR) based on surface normal,
/// directional key light (above-left), and ambient bounce light.
#[inline]
pub fn gouraud_face_colors(face_type: FaceDirection, base_tint: (u8, u8, u8)) -> [(u8, u8, u8); 4] {
    // Light intensity percentages for vertices [TL, TR, BL, BR]
    let (i0, i1, i2, i3) = match face_type {
        FaceDirection::Top => (108, 98, 98, 88), // Overhead direct sunlight
        FaceDirection::Front => (92, 85, 78, 70), // Front specular falloff
        FaceDirection::Back => (58, 50, 46, 38), // Back ambient shadow
        FaceDirection::Left => (68, 62, 56, 50), // Key-light side (distinct step from front)
        FaceDirection::Right => (54, 48, 44, 38), // Fill-light side (deep shadow)
        FaceDirection::Bottom => (38, 32, 28, 22), // Occluded ground shadow
    };

    [
        scale_rgb(base_tint, i0),
        scale_rgb(base_tint, i1),
        scale_rgb(base_tint, i2),
        scale_rgb(base_tint, i3),
    ]
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum FaceDirection {
    Top,
    Front,
    Back,
    Left,
    Right,
    Bottom,
}

#[inline]
fn scale_rgb(col: (u8, u8, u8), percent: u16) -> (u8, u8, u8) {
    let r = ((col.0 as u16 * percent) / 100).min(255) as u8;
    let g = ((col.1 as u16 * percent) / 100).min(255) as u8;
    let b = ((col.2 as u16 * percent) / 100).min(255) as u8;
    (r, g, b)
}
