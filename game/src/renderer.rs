//! Hardware GTE 3D perspective renderer, multi-theme visual environments,
//! Soliton Radar, and stage-specific HUDs for Plattypus MGS.
//!
//! Features:
//! - Full 6-sided 3D box & character rendering with proper screen-space backface culling.
//! - Accurate u16 color lighting mathematics preventing integer overflow to black.
//! - Row-based depth sorting (painter's algorithm) eliminating clipping through crates & walls.
//! - Beautiful 3D models: Platty with fur, bill, tail, webbed feet, eyes, and bandana.
//! - Act 1: Tactical compound, searchlights, sentries with rifles, Soliton Radar.
//! - Act 2: 5-lane Yarra River rapids, flowing foam, gum trees with koalas, tubers, paddle boarders, snakes, spiders.
//! - Act 3: Melbourne City Frogger, towering skyscrapers, streetlamps, taxis, trams, trucks, sports cars.
//! - Act 4: Coastal Beach 3D platformer, golden sand, surf, stepped rock ledges, bouncing parasols, beach crabs.

use crate::entities::{
    AlertState, BeachCrab, BeachPlatform, CityVehicle, Collectible, CollectibleType,
    Drone, EntityManager, PowerConduit, RiverObstacle, RiverObstacleType,
    SearchlightMech, Sentry, VehicleType,
};
use crate::level::{Act, CellType, Level, GRID_D, GRID_W, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};

use crate::texture::{FaceDirection, TextureAtlasManager, TextureId, gouraud_face_colors};

use psx_font::{fonts::BASIC, FontAtlas};
use psx_gpu::{
    self as gpu,
    framebuf::FrameBuffer,
    material::BlendMode,
    Resolution, VideoMode,
};
use psx_gte::math::{Mat3I16, Vec3I16, Vec3I32};
use psx_gte::scene;
use psx_gte_core::transform::{cos_1_3_12, sin_1_3_12};
use psx_vram::{Clut, TexDepth, Tpage};

pub const SCREEN_W: i16 = 320;
pub const SCREEN_H: i16 = 240;

const FONT_TPAGE: Tpage = Tpage::new(320, 0, TexDepth::Bit4);
const FONT_CLUT: Clut = Clut::new(320, 256);

/// Camera pitch angle: ~48 degrees downward (34 in 256-per-revolution units)
const CAM_PITCH: u16 = 34;

pub struct Renderer {
    pub fb: FrameBuffer,
    pub font: FontAtlas,
    pub textures: TextureAtlasManager,
    pub cam_x: i32,
    pub cam_y: i32,
    pub cam_z: i32,
    pub screen_shake: i16,
}

impl Renderer {
    pub fn new() -> Self {
        gpu::init(VideoMode::Ntsc, Resolution::R320X240);
        let fb = FrameBuffer::new(320, 240);
        gpu::set_draw_area(0, 0, 319, 239);
        gpu::set_draw_offset(0, 0);

        let font = FontAtlas::upload(&BASIC, FONT_TPAGE, FONT_CLUT);
        let textures = TextureAtlasManager::new();
        textures.init_and_upload();

        scene::set_screen_offset(160 << 16, 120 << 16);
        scene::set_projection_plane(200);

        Self {
            fb,
            font,
            textures,
            cam_x: 0,
            cam_y: -300,
            cam_z: -260,
            screen_shake: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        psx_rt::interrupts::wait_vblank();
        self.fb.swap();
    }

    pub fn update_camera(&mut self, player_x: i32, player_y: i32, player_z: i32) {
        let target_x = player_x;
        let target_y = player_y - 280;
        let target_z = player_z - 240;

        self.cam_x += (target_x - self.cam_x) / 4;
        self.cam_y += (target_y - self.cam_y) / 4;
        self.cam_z += (target_z - self.cam_z) / 4;

        if self.screen_shake > 0 {
            let shake = if (self.screen_shake & 1) != 0 { self.screen_shake } else { -self.screen_shake };
            self.cam_x += shake as i32;
            self.cam_z += (shake / 2) as i32;
            self.screen_shake -= 1;
        }

        let rot = Mat3I16::rotate_x(CAM_PITCH);
        let trans = rot.transform(Vec3I16::new(
            (-self.cam_x).clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            (-self.cam_y).clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            (-self.cam_z).clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        ));

        scene::load_rotation(&rot);
        scene::load_translation(Vec3I32::new(trans[0], trans[1], trans[2]));
    }

    // -------------------------------------------------------------------------
    // 3D SCENE RENDERING WITH ROW-BASED DEPTH SORTING
    // -------------------------------------------------------------------------

    pub fn draw_3d_scene(
        &self,
        level: &Level,
        platty: &Platypus,
        entities: &EntityManager,
        frame: u8,
    ) {
        // Clear backdrop tailored to stage atmosphere
        match level.act {
            Act::Act1Sanctuary | Act::Act1Boss => gpu::draw_rect_flat(0, 0, 320, 240, 10, 14, 20), // Dark military compound
            Act::Act2Bushland => gpu::draw_rect_flat(0, 0, 320, 240, 16, 40, 24),  // Yarra forest canopy
            Act::Act3City => gpu::draw_rect_flat(0, 0, 320, 240, 14, 16, 28),      // Melbourne night sky
            Act::Act4Ocean => gpu::draw_rect_flat(0, 0, 320, 240, 50, 130, 210),   // Coastal ocean sky
        }

        // Draw ground searchlights & vision cones on floor (Blended)
        if level.act == Act::Act1Sanctuary {
            self.draw_vision_cones(entities);
        } else if level.act == Act::Act1Boss {
            self.draw_boss_searchlights(&entities.boss_mech);
        }

        // Row-based depth sorting (far to near) to eliminate clipping
        let min_gx = ((self.cam_x - 320) / TILE_SZ).max(0) as usize;
        let max_gx = ((self.cam_x + 320) / TILE_SZ + 1).min(GRID_W as i32) as usize;
        let min_gz = ((self.cam_z + 80) / TILE_SZ).max(0) as usize;
        let max_gz = ((self.cam_z + 680) / TILE_SZ + 1).min(GRID_D as i32) as usize;

        for gz in min_gz..max_gz {
            let row_z_min = (gz as i32) * TILE_SZ;
            let row_z_max = ((gz + 1) as i32) * TILE_SZ;

            // 1. Draw environmental tiles in this row
            for gx in min_gx..max_gx {
                let wx = (gx as i32) * TILE_SZ;
                let wz = row_z_min;
                let cell = level.get_cell(gx, gz);
                self.draw_cell(cell, wx, wz, level.act, frame);
            }

            // 2. Draw Act-specific entities situated in this row
            match level.act {
                Act::Act1Sanctuary => {
                    for s in entities.sentries.iter() {
                        if s.active && s.z >= row_z_min && s.z < row_z_max {
                            self.draw_sentry(s, entities.frame);
                        }
                    }
                    for d in entities.drones.iter() {
                        if d.active && d.z >= row_z_min && d.z < row_z_max {
                            self.draw_drone(d, frame);
                        }
                    }
                }
                Act::Act1Boss => {
                    for c in entities.power_conduits.iter() {
                        if c.active && c.z >= row_z_min && c.z < row_z_max {
                            self.draw_power_conduit(c, frame);
                        }
                    }
                    let mech = &entities.boss_mech;
                    if mech.active && mech.z >= row_z_min && mech.z < row_z_max {
                        self.draw_searchlight_mech(mech, frame);
                    }
                }
                Act::Act2Bushland => {
                    for obs in entities.river_obstacles.iter() {
                        if obs.active && obs.z >= row_z_min && obs.z < row_z_max {
                            self.draw_river_obstacle(obs, frame);
                        }
                    }
                }
                Act::Act3City => {
                    for v in entities.vehicles.iter() {
                        if v.active && v.z >= row_z_min && v.z < row_z_max {
                            self.draw_vehicle(v);
                        }
                    }
                }
                Act::Act4Ocean => {
                    for p in entities.beach_platforms.iter() {
                        if p.active && p.z >= row_z_min && p.z < row_z_max {
                            self.draw_beach_platform(p);
                        }
                    }
                    for crab in entities.beach_crabs.iter() {
                        if crab.active && crab.z >= row_z_min && crab.z < row_z_max {
                            self.draw_crab(crab, frame);
                        }
                    }
                }
            }

            // 3. Draw Collectibles in this row
            for c in entities.collectibles.iter() {
                if c.active && c.revealed && c.z >= row_z_min && c.z < row_z_max {
                    self.draw_collectible(c, frame);
                }
            }

            // 4. Draw Plattypus if in this row
            if platty.z >= row_z_min && platty.z < row_z_max {
                self.draw_plattypus(platty);
            }
        }

        // Draw Shockwave Ground Ring if Boss Stomps
        if level.act == Act::Act1Boss && entities.boss_mech.shockwave_active {
            self.draw_shockwave_ring(&entities.boss_mech);
        }

        // Draw 3D Particles
        self.draw_particles(entities);
    }

    fn draw_gum_tree(&self, wx: i32, wz: i32) {
        let rot = Mat3I16::rotate_y(0);
        // Tall eucalyptus trunk
        self.draw_model_box(wx + 26, 0, wz + 26, -5, -46, -5, 10, 46, 10, &rot, (175, 170, 155));
        // Gum leaf canopy
        self.draw_model_box(wx + 26, 0, wz + 26, -18, -66, -18, 36, 22, 36, &rot, (45, 100, 48));
        self.draw_model_box(wx + 26, 0, wz + 26, -12, -76, -12, 24, 12, 24, &rot, (65, 130, 60));
    }

    fn draw_streetlamp(&self, wx: i32, wz: i32) {
        let rot = Mat3I16::rotate_y(0);
        // Sleek dark lamp post
        self.draw_model_box(wx + 8, 0, wz + 8, -2, -52, -2, 4, 52, 4, &rot, (45, 50, 58));
        // Overhanging lamp arm
        self.draw_model_box(wx + 8, 0, wz + 8, -2, -54, 2, 4, 4, 10, &rot, (45, 50, 58));
        // Glowing warm yellow light fixture
        self.draw_model_box(wx + 8, 0, wz + 8, -3, -51, 8, 6, 4, 6, &rot, (255, 245, 140));
        // Warm light puddle on asphalt
        let lp = scene::project_vertex(Vec3I16::new((wx + 8) as i16, 0, (wz + 14) as i16));
        if lp.sz > 20 {
            gpu::draw_rect_flat(lp.sx - 8, lp.sy - 4, 16, 8, 85, 80, 50);
        }
    }

    fn draw_palm_tree(&self, wx: i32, wz: i32) {
        let rot = Mat3I16::rotate_y(0);
        // Curved brown trunk
        self.draw_model_box(wx + 28, 0, wz + 28, -4, -42, -4, 8, 42, 8, &rot, (140, 95, 55));
        // Tropical palm fronds canopy
        self.draw_model_box(wx + 28, 0, wz + 28, -22, -50, -22, 44, 10, 44, &rot, (42, 165, 58));
        self.draw_model_box(wx + 28, 0, wz + 28, -14, -56, -14, 28, 8, 28, &rot, (70, 200, 80));
    }

    fn draw_cell(&self, cell: CellType, wx: i32, wz: i32, act: Act, frame: u8) {
        let (floor_r, floor_g, floor_b) = match act {
            Act::Act1Sanctuary | Act::Act1Boss => (30, 38, 44),   // Dark tarmac / concrete
            Act::Act2Bushland => (25, 75, 40),    // Lush Yarra riverbank moss & grass
            Act::Act3City => (48, 52, 58),        // City asphalt road
            Act::Act4Ocean => (220, 200, 145),    // Warm coastal sand
        };

        let gx = (wx / TILE_SZ) as usize;
        let gz = (wz / TILE_SZ) as usize;

        match cell {
            CellType::Floor => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                match act {
                    Act::Act3City => {
                        // Road dividing dashed white lines
                        if gz == 18 || gz == 12 || gz == 6 {
                            self.draw_box_3d(wx + 24, wz + 28, 16, 1, 8, (230, 230, 230));
                        }
                        // Sidewalk curbs and streetlamps on pedestrian boundaries
                        if (gx == 4 || gx == 19) && (gz % 4 == 0) {
                            self.draw_streetlamp(wx, wz);
                        }
                    }
                    Act::Act4Ocean => {
                        // Palm trees on outer dunes
                        if (gx == 1 || gx == 22) && (gz % 3 == 0) {
                            self.draw_palm_tree(wx, wz);
                        }
                    }
                    _ => {}
                }
            }
            CellType::TallGrass => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                self.draw_grass_clump(wx + 16, wz + 16);
                self.draw_grass_clump(wx + 44, wz + 36);
            }
            CellType::Water => {
                self.draw_water_tile(wx, wz, act, frame, false);
            }
            CellType::WaterCurrent => {
                self.draw_water_tile(wx, wz, act, frame, true);
            }
            CellType::Crate => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                // 48x48x48 cargo crate with texture & Gouraud shading
                self.draw_box_3d_textured(wx + 8, wz + 8, 48, 48, 48, TextureId::Crate, (180, 180, 180));
            }
            CellType::Container => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                let (c_tex, c_col, h) = match act {
                    Act::Act1Sanctuary | Act::Act1Boss => (TextureId::ConcreteWall, (150, 150, 150), 70),
                    Act::Act2Bushland => (TextureId::RiverLog, (140, 140, 140), 64),
                    Act::Act3City => (TextureId::CityBrick, (160, 160, 160), 110), // Giant illuminated skyscraper!
                    Act::Act4Ocean => (TextureId::RockCliff, (160, 160, 160), 55), // Stepped sandcliff
                };
                self.draw_box_3d_textured(wx + 2, wz + 2, 60, h, 60, c_tex, c_col);

                // Add glowing windows and beacons on city skyscrapers
                if act == Act::Act3City {
                    let win_col = if (wx / 64) % 2 == 0 { (255, 230, 110) } else { (100, 210, 255) };
                    self.draw_box_3d(wx + 10, wz + 4, 12, 16, 2, win_col);
                    self.draw_box_3d(wx + 38, wz + 4, 12, 16, 2, win_col);
                    if gz % 2 == 0 {
                        let beacon_col = if (frame / 16) % 2 == 0 { (255, 40, 40) } else { (255, 120, 40) };
                        self.draw_box_3d(wx + 26, wz + 26, 8, 12, 8, beacon_col);
                    }
                }
            }
            CellType::Wall => {
                match act {
                    Act::Act2Bushland => {
                        // Bushland riverbank: mossy bank + gum tree
                        self.draw_box_3d_textured(wx, wz, 64, 32, 64, TextureId::GumLeaves, (140, 140, 140));
                        if (gx + gz) % 2 == 0 {
                            self.draw_gum_tree(wx, wz);
                        }
                    }
                    Act::Act1Sanctuary | Act::Act1Boss => {
                        self.draw_box_3d_textured(wx, wz, 64, 80, 64, TextureId::ConcreteWall, (150, 150, 150));
                    }
                    Act::Act3City => {
                        self.draw_box_3d_textured(wx, wz, 64, 80, 64, TextureId::CityBrick, (160, 160, 160));
                    }
                    Act::Act4Ocean => {
                        self.draw_box_3d_textured(wx, wz, 64, 60, 64, TextureId::RockCliff, (160, 160, 160));
                    }
                }
            }
            CellType::AirDuct => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                // Low ventilation shaft passable only when crawling
                self.draw_box_3d_textured(wx + 8, wz + 8, 48, 24, 48, TextureId::MetalGrate, (150, 150, 150));
            }
            CellType::LaserTripwire => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                self.draw_laser_tripwire(wx, wz, frame);
            }
            CellType::ExitBurrow => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                self.draw_exit_hatch(wx + 12, wz + 12, frame);
            }
        }
    }

    fn draw_floor_tile(&self, wx: i32, wz: i32, r: u8, g: u8, b: u8, act: Act) {
        let v0 = Vec3I16::new(wx as i16, 0, wz as i16);
        let v1 = Vec3I16::new((wx + TILE_SZ) as i16, 0, wz as i16);
        let v2 = Vec3I16::new(wx as i16, 0, (wz + TILE_SZ) as i16);
        let v3 = Vec3I16::new((wx + TILE_SZ) as i16, 0, (wz + TILE_SZ) as i16);

        let floor_tex = match act {
            Act::Act1Sanctuary | Act::Act1Boss => TextureId::MetalGrate,
            Act::Act2Bushland => TextureId::GumLeaves,
            Act::Act3City => TextureId::CityAsphalt,
            Act::Act4Ocean => TextureId::BeachSand,
        };
        self.draw_quad_3d_textured_gouraud(v0, v1, v2, v3, floor_tex, FaceDirection::Top, (r, g, b));
    }

    fn draw_water_tile(&self, wx: i32, wz: i32, act: Act, frame: u8, is_current: bool) {
        let bob = if (frame / 8) % 2 == 0 { 2 } else { 0 };
        let y = 14 + bob;
        let v0 = Vec3I16::new(wx as i16, y, wz as i16);
        let v1 = Vec3I16::new((wx + TILE_SZ) as i16, y, wz as i16);
        let v2 = Vec3I16::new(wx as i16, y, (wz + TILE_SZ) as i16);
        let v3 = Vec3I16::new((wx + TILE_SZ) as i16, y, (wz + TILE_SZ) as i16);

        let tint = match act {
            Act::Act4Ocean => (160, 200, 240),
            _ => {
                if is_current {
                    (190, 220, 255)
                } else {
                    (150, 180, 220)
                }
            }
        };
        self.draw_quad_3d_textured_gouraud(v0, v1, v2, v3, TextureId::RiverWater, FaceDirection::Top, tint);

        // Animated foam ripples
        if is_current || act == Act::Act4Ocean {
            let foam_z = (wz + ((frame as i32 * 3) % TILE_SZ)) as i16;
            let p0 = scene::project_vertex(Vec3I16::new(wx as i16 + 8, y - 1, foam_z));
            let p1 = scene::project_vertex(Vec3I16::new(wx as i16 + 56, y - 1, foam_z));
            if p0.sz > 20 && p1.sz > 20 {
                gpu::draw_line_mono(p0.sx, p0.sy, p1.sx, p1.sy, 220, 240, 255);
            }
        }
    }

    fn draw_grass_clump(&self, wx: i32, wz: i32) {
        let p0 = scene::project_vertex(Vec3I16::new((wx - 12) as i16, 0, wz as i16));
        let p1 = scene::project_vertex(Vec3I16::new((wx + 12) as i16, 0, wz as i16));
        let p2 = scene::project_vertex(Vec3I16::new(wx as i16, -22, wz as i16));
        if p0.sz > 20 && p1.sz > 20 && p2.sz > 20 {
            gpu::draw_tri_flat([(p0.sx, p0.sy), (p1.sx, p1.sy), (p2.sx, p2.sy)], 50, 135, 60);
        }
    }

    fn draw_laser_tripwire(&self, wx: i32, wz: i32, frame: u8) {
        self.draw_box_3d(wx + 2, wz + 28, 8, 24, 8, (90, 95, 105));
        self.draw_box_3d(wx + 54, wz + 28, 8, 24, 8, (90, 95, 105));

        let p0 = scene::project_vertex(Vec3I16::new((wx + 6) as i16, -10, (wz + 32) as i16));
        let p1 = scene::project_vertex(Vec3I16::new((wx + 58) as i16, -10, (wz + 32) as i16));
        if p0.sz > 20 && p1.sz > 20 {
            let glow = if (frame / 4) % 2 == 0 { 255 } else { 180 };
            gpu::draw_line_mono(p0.sx, p0.sy, p1.sx, p1.sy, glow, 30, 40);
        }
    }

    fn draw_exit_hatch(&self, wx: i32, wz: i32, frame: u8) {
        let v0 = Vec3I16::new(wx as i16, 0, wz as i16);
        let v1 = Vec3I16::new((wx + 40) as i16, 0, wz as i16);
        let v2 = Vec3I16::new(wx as i16, 0, (wz + 40) as i16);
        let v3 = Vec3I16::new((wx + 40) as i16, 0, (wz + 40) as i16);
        Self::draw_quad_3d(v0, v1, v2, v3, 130, 95, 60);

        let p = scene::project_vertex(Vec3I16::new((wx + 20) as i16, -18, (wz + 20) as i16));
        if p.sz > 20 {
            let spark = if (frame / 8) % 2 == 0 { 255 } else { 180 };
            gpu::draw_rect_flat(p.sx - 5, p.sy - 5, 10, 10, spark, spark, 90);
        }
    }

    /// Draw a full 6-sided 3D box with directional Gouraud lighting and texture mapping.
    pub fn draw_box_3d(&self, wx: i32, wz: i32, w: i32, h: i32, d: i32, col: (u8, u8, u8)) {
        self.draw_box_3d_textured(wx, wz, w, h, d, TextureId::Crate, col);
    }

    /// Draw a full 6-sided 3D box with textured quad Gouraud shading and backface culling.
    pub fn draw_box_3d_textured(
        &self,
        wx: i32,
        wz: i32,
        w: i32,
        h: i32,
        d: i32,
        texture: TextureId,
        col: (u8, u8, u8),
    ) {
        let x0 = wx as i16;
        let x1 = (wx + w) as i16;
        let y0 = -h as i16;
        let y1 = 0i16;
        let z0 = wz as i16;
        let z1 = (wz + d) as i16;

        // 1. TOP FACE (lit by overhead sunlight)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x0, y0, z0),
            Vec3I16::new(x1, y0, z0),
            Vec3I16::new(x0, y0, z1),
            Vec3I16::new(x1, y0, z1),
            texture,
            FaceDirection::Top,
            col,
        );

        // 2. FRONT FACE (facing +Z)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x0, y0, z1),
            Vec3I16::new(x1, y0, z1),
            Vec3I16::new(x0, y1, z1),
            Vec3I16::new(x1, y1, z1),
            texture,
            FaceDirection::Front,
            col,
        );

        // 3. BACK FACE (facing -Z)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x1, y0, z0),
            Vec3I16::new(x0, y0, z0),
            Vec3I16::new(x1, y1, z0),
            Vec3I16::new(x0, y1, z0),
            texture,
            FaceDirection::Back,
            col,
        );

        // 4. LEFT FACE (facing -X)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x0, y0, z0),
            Vec3I16::new(x0, y0, z1),
            Vec3I16::new(x0, y1, z0),
            Vec3I16::new(x0, y1, z1),
            texture,
            FaceDirection::Left,
            col,
        );

        // 5. RIGHT FACE (facing +X)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x1, y0, z1),
            Vec3I16::new(x1, y0, z0),
            Vec3I16::new(x1, y1, z1),
            Vec3I16::new(x1, y1, z0),
            texture,
            FaceDirection::Right,
            col,
        );

        // 6. BOTTOM FACE (facing +Y, ground shadow)
        self.draw_quad_3d_textured_gouraud(
            Vec3I16::new(x0, y1, z1),
            Vec3I16::new(x1, y1, z1),
            Vec3I16::new(x0, y1, z0),
            Vec3I16::new(x1, y1, z0),
            texture,
            FaceDirection::Bottom,
            col,
        );
    }

    /// Helper to draw a local 3D box transformed by a rotation and placed at world position.
    pub fn draw_model_box(
        &self,
        wx: i32,
        wy: i32,
        wz: i32,
        lx: i32,
        ly: i32,
        lz: i32,
        w: i32,
        h: i32,
        d: i32,
        rot: &Mat3I16,
        col: (u8, u8, u8),
    ) {
        self.draw_model_box_textured(wx, wy, wz, lx, ly, lz, w, h, d, rot, None, col);
    }

    /// Helper to draw a textured 3D model box with Gouraud vertex shading.
    pub fn draw_model_box_textured(
        &self,
        wx: i32,
        wy: i32,
        wz: i32,
        lx: i32,
        ly: i32,
        lz: i32,
        w: i32,
        h: i32,
        d: i32,
        rot: &Mat3I16,
        texture: Option<TextureId>,
        col: (u8, u8, u8),
    ) {
        let corners = [
            (lx, ly, lz),             // 0: Top, Back, Left
            (lx + w, ly, lz),         // 1: Top, Back, Right
            (lx, ly, lz + d),         // 2: Top, Front, Left
            (lx + w, ly, lz + d),     // 3: Top, Front, Right
            (lx, ly + h, lz),         // 4: Bottom, Back, Left
            (lx + w, ly + h, lz),     // 5: Bottom, Back, Right
            (lx, ly + h, lz + d),     // 6: Bottom, Front, Left
            (lx + w, ly + h, lz + d), // 7: Bottom, Front, Right
        ];

        let mut world_pts = [Vec3I16::ZERO; 8];
        for (i, c) in corners.iter().enumerate() {
            let t = rot.transform(Vec3I16::new(c.0 as i16, c.1 as i16, c.2 as i16));
            world_pts[i] = Vec3I16::new(
                (wx + t[0]) as i16,
                (wy + t[1]) as i16,
                (wz + t[2]) as i16,
            );
        }

        if let Some(tex) = texture {
            self.draw_quad_3d_textured_gouraud(world_pts[0], world_pts[1], world_pts[2], world_pts[3], tex, FaceDirection::Top, col);
            self.draw_quad_3d_textured_gouraud(world_pts[2], world_pts[3], world_pts[6], world_pts[7], tex, FaceDirection::Front, col);
            self.draw_quad_3d_textured_gouraud(world_pts[1], world_pts[0], world_pts[5], world_pts[4], tex, FaceDirection::Back, col);
            self.draw_quad_3d_textured_gouraud(world_pts[2], world_pts[0], world_pts[6], world_pts[4], tex, FaceDirection::Left, col);
            self.draw_quad_3d_textured_gouraud(world_pts[1], world_pts[3], world_pts[5], world_pts[7], tex, FaceDirection::Right, col);
            self.draw_quad_3d_textured_gouraud(world_pts[6], world_pts[7], world_pts[4], world_pts[5], tex, FaceDirection::Bottom, col);
        } else {
            let f_col = ((col.0 as u16 * 85 / 100) as u8, (col.1 as u16 * 85 / 100) as u8, (col.2 as u16 * 85 / 100) as u8);
            let b_col = ((col.0 as u16 * 75 / 100) as u8, (col.1 as u16 * 75 / 100) as u8, (col.2 as u16 * 75 / 100) as u8);
            let l_col = ((col.0 as u16 * 70 / 100) as u8, (col.1 as u16 * 70 / 100) as u8, (col.2 as u16 * 70 / 100) as u8);
            let r_col = ((col.0 as u16 * 60 / 100) as u8, (col.1 as u16 * 60 / 100) as u8, (col.2 as u16 * 60 / 100) as u8);
            let bot_col = ((col.0 as u16 * 45 / 100) as u8, (col.1 as u16 * 45 / 100) as u8, (col.2 as u16 * 45 / 100) as u8);

            Self::draw_quad_3d(world_pts[0], world_pts[1], world_pts[2], world_pts[3], col.0, col.1, col.2);
            Self::draw_quad_3d(world_pts[2], world_pts[3], world_pts[6], world_pts[7], f_col.0, f_col.1, f_col.2);
            Self::draw_quad_3d(world_pts[1], world_pts[0], world_pts[5], world_pts[4], b_col.0, b_col.1, b_col.2);
            Self::draw_quad_3d(world_pts[2], world_pts[0], world_pts[6], world_pts[4], l_col.0, l_col.1, l_col.2);
            Self::draw_quad_3d(world_pts[1], world_pts[3], world_pts[5], world_pts[7], r_col.0, r_col.1, r_col.2);
            Self::draw_quad_3d(world_pts[6], world_pts[7], world_pts[4], world_pts[5], bot_col.0, bot_col.1, bot_col.2);
        }
    }

    /// Project and render a 3D quad using hardware textured Gouraud primitive GP0(0x3C).
    #[inline]
    pub fn draw_quad_3d_textured_gouraud(
        &self,
        v0: Vec3I16,
        v1: Vec3I16,
        v2: Vec3I16,
        v3: Vec3I16,
        texture: TextureId,
        face_dir: FaceDirection,
        base_tint: (u8, u8, u8),
    ) {
        let p0 = scene::project_vertex(v0);
        let p1 = scene::project_vertex(v1);
        let p2 = scene::project_vertex(v2);
        let p3 = scene::project_vertex(v3);

        if p0.sz < 20 || p1.sz < 20 || p2.sz < 20 || p3.sz < 20 {
            return;
        }

        // Screen-space backface culling check
        let ax = p1.sx as i32 - p0.sx as i32;
        let ay = p1.sy as i32 - p0.sy as i32;
        let bx = p2.sx as i32 - p0.sx as i32;
        let by = p2.sy as i32 - p0.sy as i32;
        if ax * by - ay * bx <= 0 {
            return;
        }

        let (bank, uvs) = texture.uv_and_bank();
        let clut_word = self.textures.clut_words[bank as usize];
        let tpage_word = self.textures.tpage_word;
        let colors = gouraud_face_colors(face_dir, base_tint);

        gpu::draw_quad_textured_gouraud(
            [(p0.sx, p0.sy), (p1.sx, p1.sy), (p2.sx, p2.sy), (p3.sx, p3.sy)],
            uvs,
            colors,
            clut_word,
            tpage_word,
        );
    }

    /// Project and render a 3D quad using native PS1 GPU hardware flat quad GP0(0x28).
    #[inline]
    pub fn draw_quad_3d(v0: Vec3I16, v1: Vec3I16, v2: Vec3I16, v3: Vec3I16, r: u8, g: u8, b: u8) {
        let p0 = scene::project_vertex(v0);
        let p1 = scene::project_vertex(v1);
        let p2 = scene::project_vertex(v2);
        let p3 = scene::project_vertex(v3);

        if p0.sz < 20 || p1.sz < 20 || p2.sz < 20 || p3.sz < 20 {
            return;
        }

        // Screen-space backface culling check
        let ax = p1.sx as i32 - p0.sx as i32;
        let ay = p1.sy as i32 - p0.sy as i32;
        let bx = p2.sx as i32 - p0.sx as i32;
        let by = p2.sy as i32 - p0.sy as i32;
        if ax * by - ay * bx <= 0 {
            return;
        }

        gpu::draw_quad_flat([(p0.sx, p0.sy), (p1.sx, p1.sy), (p2.sx, p2.sy), (p3.sx, p3.sy)], r, g, b);
    }

    // -------------------------------------------------------------------------
    // VISION CONES & SEARCHLIGHTS (ACT 1)
    // -------------------------------------------------------------------------

    fn draw_vision_cones(&self, entities: &EntityManager) {
        for s in entities.searchlights.iter() {
            if !s.active {
                continue;
            }
            let rad = s.radius as i16;
            let bx = s.beam_x as i16;
            let bz = s.beam_z as i16;

            let c0 = scene::project_vertex(Vec3I16::new(bx - rad, 1, bz));
            let c1 = scene::project_vertex(Vec3I16::new(bx, 1, bz - rad));
            let c2 = scene::project_vertex(Vec3I16::new(bx + rad, 1, bz));
            let c3 = scene::project_vertex(Vec3I16::new(bx, 1, bz + rad));

            if c0.sz > 20 && c1.sz > 20 && c2.sz > 20 && c3.sz > 20 {
                gpu::draw_tri_flat_blended([(c0.sx, c0.sy), (c1.sx, c1.sy), (c2.sx, c2.sy)], 110, 110, 60, BlendMode::Add);
                gpu::draw_tri_flat_blended([(c0.sx, c0.sy), (c2.sx, c2.sy), (c3.sx, c3.sy)], 110, 110, 60, BlendMode::Add);
            }
        }

        for s in entities.sentries.iter() {
            if !s.active || s.stun_timer > 0 {
                continue;
            }

            let sx = s.x as i16;
            let sz = s.z as i16;

            let left_ang = s.angle.wrapping_sub(24);
            let right_ang = s.angle.wrapping_add(24);

            let l_x = sx + ((sin_1_3_12(left_ang) as i32 * 180) >> 12) as i16;
            let l_z = sz + ((cos_1_3_12(left_ang) as i32 * 180) >> 12) as i16;
            let r_x = sx + ((sin_1_3_12(right_ang) as i32 * 180) >> 12) as i16;
            let r_z = sz + ((cos_1_3_12(right_ang) as i32 * 180) >> 12) as i16;

            let p_origin = scene::project_vertex(Vec3I16::new(sx, 1, sz));
            let p_left = scene::project_vertex(Vec3I16::new(l_x, 1, l_z));
            let p_right = scene::project_vertex(Vec3I16::new(r_x, 1, r_z));

            if p_origin.sz > 20 && p_left.sz > 20 && p_right.sz > 20 {
                let (cr, cg, cb) = if s.see_player {
                    (220, 30, 30) // Red alert!
                } else {
                    (40, 140, 65) // Green stealth
                };

                gpu::draw_tri_flat_blended(
                    [(p_origin.sx, p_origin.sy), (p_left.sx, p_left.sy), (p_right.sx, p_right.sy)],
                    cr, cg, cb, BlendMode::Add,
                );
            }
        }
    }

    fn draw_boss_searchlights(&self, mech: &SearchlightMech) {
        if !mech.active {
            return;
        }

        // Left searchlight ground pool
        let rad = 42i16;
        let lx = mech.left_beam_x as i16;
        let lz = mech.left_beam_z as i16;

        let lc0 = scene::project_vertex(Vec3I16::new(lx - rad, 1, lz));
        let lc1 = scene::project_vertex(Vec3I16::new(lx, 1, lz - rad));
        let lc2 = scene::project_vertex(Vec3I16::new(lx + rad, 1, lz));
        let lc3 = scene::project_vertex(Vec3I16::new(lx, 1, lz + rad));

        if lc0.sz > 20 && lc1.sz > 20 && lc2.sz > 20 && lc3.sz > 20 {
            gpu::draw_tri_flat_blended([(lc0.sx, lc0.sy), (lc1.sx, lc1.sy), (lc2.sx, lc2.sy)], 140, 140, 70, BlendMode::Add);
            gpu::draw_tri_flat_blended([(lc0.sx, lc0.sy), (lc2.sx, lc2.sy), (lc3.sx, lc3.sy)], 140, 140, 70, BlendMode::Add);
        }

        // Right searchlight ground pool
        let rx = mech.right_beam_x as i16;
        let rz = mech.right_beam_z as i16;

        let rc0 = scene::project_vertex(Vec3I16::new(rx - rad, 1, rz));
        let rc1 = scene::project_vertex(Vec3I16::new(rx, 1, rz - rad));
        let rc2 = scene::project_vertex(Vec3I16::new(rx + rad, 1, rz));
        let rc3 = scene::project_vertex(Vec3I16::new(rx, 1, rz + rad));

        if rc0.sz > 20 && rc1.sz > 20 && rc2.sz > 20 && rc3.sz > 20 {
            gpu::draw_tri_flat_blended([(rc0.sx, rc0.sy), (rc1.sx, rc1.sy), (rc2.sx, rc2.sy)], 140, 140, 70, BlendMode::Add);
            gpu::draw_tri_flat_blended([(rc0.sx, rc0.sy), (rc2.sx, rc2.sy), (rc3.sx, rc3.sy)], 140, 140, 70, BlendMode::Add);
        }

        // Volumetric light beam quad from mech shoulder projectors to ground
        let l_pod = scene::project_vertex(Vec3I16::new((mech.x - 24) as i16, (mech.y - 48) as i16, mech.z as i16));
        if l_pod.sz > 20 && lc1.sz > 20 && lc3.sz > 20 {
            gpu::draw_tri_flat_blended([(l_pod.sx, l_pod.sy), (lc1.sx, lc1.sy), (lc3.sx, lc3.sy)], 70, 70, 35, BlendMode::Add);
        }

        let r_pod = scene::project_vertex(Vec3I16::new((mech.x + 24) as i16, (mech.y - 48) as i16, mech.z as i16));
        if r_pod.sz > 20 && rc1.sz > 20 && rc3.sz > 20 {
            gpu::draw_tri_flat_blended([(r_pod.sx, r_pod.sy), (rc1.sx, rc1.sy), (rc3.sx, rc3.sy)], 70, 70, 35, BlendMode::Add);
        }
    }

    fn draw_shockwave_ring(&self, mech: &SearchlightMech) {
        if !mech.shockwave_active || mech.shockwave_radius <= 0 {
            return;
        }

        let rad = mech.shockwave_radius;
        let first_ang: u16 = 0;
        let first_x = mech.x + ((cos_1_3_12(first_ang) as i32 * rad) >> 12);
        let first_z = mech.z + ((sin_1_3_12(first_ang) as i32 * rad) >> 12);
        let mut prev_p = scene::project_vertex(Vec3I16::new(first_x as i16, 0, first_z as i16));

        for step in 1..=12 {
            let ang = (step * 21) as u16; // 0..256 circle
            let px = mech.x + ((cos_1_3_12(ang) as i32 * rad) >> 12);
            let pz = mech.z + ((sin_1_3_12(ang) as i32 * rad) >> 12);
            let cur_p = scene::project_vertex(Vec3I16::new(px as i16, 0, pz as i16));

            if prev_p.sz > 20 && cur_p.sz > 20 {
                gpu::draw_line_mono(prev_p.sx, prev_p.sy, cur_p.sx, cur_p.sy, 240, 220, 160);
            }
            prev_p = cur_p;
        }
    }

    fn draw_searchlight_mech(&self, mech: &SearchlightMech, frame: u8) {
        let mx = mech.x;
        let my = mech.y;
        let mz = mech.z;
        let rot = Mat3I16::rotate_y(mech.angle);

        let is_hit = mech.hit_timer > 0 && ((mech.hit_timer / 2) % 2 == 1);
        let armor_col = if is_hit {
            (255, 255, 200)
        } else {
            (65, 75, 60) // Military olive drab
        };
        let dark_metal = (40, 45, 45);

        // Ground shadow beneath mech
        let sp = scene::project_vertex(Vec3I16::new(mx as i16, 0, mz as i16));
        if sp.sz > 20 {
            gpu::draw_rect_flat(sp.sx - 24, sp.sy - 8, 48, 16, 12, 16, 20);
        }

        // Bipedal Walking Legs
        let leg_swing = match mech.leg_anim {
            0 => 6,
            1 => 0,
            2 => -6,
            _ => 0,
        };

        // Left Leg: Hip, Thigh, Calf, Foot
        self.draw_model_box(mx, my, mz, -22, -26, leg_swing - 4, 8, 16, 8, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, -22, -12, -leg_swing - 4, 8, 14, 8, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, -24, 0, -leg_swing - 6, 12, 4, 16, &rot, armor_col);

        // Right Leg: Hip, Thigh, Calf, Foot
        self.draw_model_box(mx, my, mz, 14, -26, -leg_swing - 4, 8, 16, 8, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, 14, -12, leg_swing - 4, 8, 14, 8, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, 12, 0, leg_swing - 6, 12, 4, 16, &rot, armor_col);

        // Armoured Torso / Cockpit
        self.draw_model_box_textured(mx, my, mz, -20, -52, -14, 40, 28, 28, &rot, Some(TextureId::Crate), armor_col);

        // Reinforced Front Cockpit Visor / Armor Slit
        let visor_col = if is_hit { (255, 255, 255) } else { (220, 50, 40) };
        self.draw_model_box(mx, my, mz, -14, -44, 14, 28, 8, 4, &rot, visor_col);

        // Left Shoulder Searchlight Pod
        self.draw_model_box(mx, my, mz, -28, -50, -4, 8, 14, 12, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, -28, -48, 8, 8, 10, 2, &rot, (255, 250, 160)); // Lens

        // Right Shoulder Searchlight Pod
        self.draw_model_box(mx, my, mz, 20, -50, -4, 8, 14, 12, &rot, dark_metal);
        self.draw_model_box(mx, my, mz, 20, -48, 8, 8, 10, 2, &rot, (255, 250, 160)); // Lens

        // Rear Heat Coolant Core & Energy Shield
        if mech.shield_active {
            // Glowing Energy Shield surrounding rear core
            let shield_glow = if (frame / 4) % 2 == 0 { 240 } else { 160 };
            self.draw_model_box(mx, my, mz, -12, -46, -22, 24, 20, 8, &rot, (50, 120, shield_glow));
        } else {
            // Overheated / Vulnerable Exposed Core!
            let pulse = if (frame / 3) % 2 == 0 { 255 } else { 140 };
            self.draw_model_box(mx, my, mz, -10, -44, -22, 20, 16, 8, &rot, (pulse, 80, 20));
            // Radiator fins
            self.draw_model_box(mx, my, mz, -8, -42, -24, 16, 12, 2, &rot, (255, 220, 40));
        }
    }

    fn draw_power_conduit(&self, conduit: &PowerConduit, frame: u8) {
        let cx = conduit.x;
        let cz = conduit.z;
        let rot = Mat3I16::rotate_y(0);

        if conduit.destroyed {
            // Ruined blackened scrap pylon
            self.draw_model_box(cx, 0, cz, -10, -10, -10, 20, 10, 20, &rot, (35, 35, 40));
            self.draw_model_box(cx, 0, cz, -5, -18, -5, 10, 8, 10, &rot, (25, 25, 28));
        } else {
            // Heavy concrete pedestal
            self.draw_model_box_textured(cx, 0, cz, -12, -10, -12, 24, 10, 24, &rot, Some(TextureId::Crate), (70, 75, 80));

            // Central Transformer Core
            self.draw_model_box(cx, 0, cz, -6, -32, -6, 12, 22, 12, &rot, (45, 50, 60));

            // High-voltage glowing coil
            let glow = if (frame / 4) % 2 == 0 { 255 } else { 180 };
            self.draw_model_box(cx, 0, cz, -8, -26, -8, 16, 12, 16, &rot, (60, 200, glow));

            // Insulator cap
            self.draw_model_box(cx, 0, cz, -10, -36, -10, 20, 4, 20, &rot, (90, 95, 105));

            // Lightning spark arc at top of pylon
            let p = scene::project_vertex(Vec3I16::new(cx as i16, -38, cz as i16));
            if p.sz > 20 {
                let spark = if (frame / 2) % 2 == 0 { 255 } else { 80 };
                gpu::draw_rect_flat(p.sx - 3, p.sy - 3, 6, 6, spark, 230, 255);
            }
        }
    }

    fn draw_boss_hud(&self, mech: &SearchlightMech, conduits: &[PowerConduit; 3]) {
        // Dark tactical frame
        let bx: i16 = 180;
        let by: i16 = 8;
        let bw: u16 = 134;
        let bh: u16 = 36;

        gpu::draw_rect_flat(bx, by, bw, bh, 12, 18, 24);
        gpu::draw_rect_flat(bx + 2, by + 2, bw - 4, bh - 4, 4, 8, 12);

        // Header: Boss name
        self.font.draw_text(bx + 6, by + 4, "SEARCHLIGHT MECH", (255, 60, 60));

        if mech.shield_active {
            self.font.draw_text(bx + 6, by + 18, "SHIELD", (80, 210, 255));
            // 3 Conduit Power Matrix Cells
            for i in 0..3 {
                let col = if !conduits[i].destroyed {
                    (80, 220, 255) // Active cyan
                } else {
                    (60, 25, 25)   // Shattered dark red
                };
                let cx = bx + 58 + (i as i16 * 24);
                gpu::draw_rect_flat(cx, by + 19, 20, 8, col.0, col.1, col.2);
            }
        } else {
            self.font.draw_text(bx + 6, by + 18, "CORE HP", (255, 180, 40));
            // 4 Segmented HP bars for the vulnerable coolant core
            for i in 0..4 {
                let col = if (i as u8) < mech.health {
                    (255, 50, 40) // Bright orange-red
                } else {
                    (45, 15, 15)  // Depleted dark
                };
                let cx = bx + 64 + (i as i16 * 16);
                gpu::draw_rect_flat(cx, by + 19, 13, 8, col.0, col.1, col.2);
            }
        }
    }

    // -------------------------------------------------------------------------
    // 3D PLATYPUS CHARACTER MODEL
    // -------------------------------------------------------------------------

    fn draw_plattypus(&self, platty: &Platypus) {
        if platty.invuln_timer > 0 && (platty.invuln_timer / 4) % 2 == 1 {
            return;
        }

        let px = platty.x;
        let py = platty.y;
        let pz = platty.z;
        let rot = Mat3I16::rotate_y(platty.angle);

        // Soft ground shadow beneath Platty
        let shadow_p = scene::project_vertex(Vec3I16::new(px as i16, 0, pz as i16));
        if shadow_p.sz > 20 {
            gpu::draw_rect_flat(shadow_p.sx - 8, shadow_p.sy - 3, 16, 6, 20, 24, 30);
        }

        let is_crawl = platty.state == PlayerState::BellyCrawl;
        let body_h = if is_crawl { 8 } else { 16 };

        // 1. Platty Body (Rich warm chestnut brown fur with 4bpp pelt texture & Gouraud shading)
        self.draw_model_box_textured(px, py, pz, -12, -body_h, -14, 24, body_h, 28, &rot, Some(TextureId::PlattyFur), (160, 110, 60));

        // 2. Underbelly Cream
        self.draw_model_box_textured(px, py, pz, -8, -body_h / 2, -10, 16, body_h / 2, 20, &rot, Some(TextureId::PlattyFur), (200, 150, 100));

        // 3. Duck Bill (+Z in local facing) with leathery sensory texture
        self.draw_model_box_textured(px, py, pz, -7, -8, 14, 14, 5, 16, &rot, Some(TextureId::PlattyBill), (150, 150, 150));

        // 4. Beaver Paddle Tail (-Z in local facing)
        let tail_wobble = ((platty.anim_frame / 4) % 2) as i32 * 2;
        self.draw_model_box_textured(px, py, pz, -9 + tail_wobble, -5, -30, 18, 5, 18, &rot, Some(TextureId::PlattyFur), (130, 80, 45));

        // 5. Webbed Feet (Orange-tan)
        self.draw_model_box(px, py, pz, -15, -3, 6, 5, 3, 8, &rot, (215, 130, 45));
        self.draw_model_box(px, py, pz, 10, -3, 6, 5, 3, 8, &rot, (215, 130, 45));
        self.draw_model_box(px, py, pz, -15, -3, -12, 5, 3, 8, &rot, (215, 130, 45));
        self.draw_model_box(px, py, pz, 10, -3, -12, 5, 3, 8, &rot, (215, 130, 45));

        // 6. Expressive Eyes
        self.draw_model_box(px, py, pz, -8, -body_h - 2, 8, 4, 4, 4, &rot, (255, 255, 255));
        self.draw_model_box(px, py, pz, -8, -body_h - 1, 11, 2, 2, 2, &rot, (20, 20, 20));
        self.draw_model_box(px, py, pz, 4, -body_h - 2, 8, 4, 4, 4, &rot, (255, 255, 255));
        self.draw_model_box(px, py, pz, 6, -body_h - 1, 11, 2, 2, 2, &rot, (20, 20, 20));

        // 7. Leather Satchel with Important Letter!
        self.draw_model_box(px, py, pz, 5, -body_h - 3, -4, 9, 8, 10, &rot, (185, 135, 65));
        self.draw_model_box(px, py, pz, 7, -body_h - 5, -2, 5, 3, 6, &rot, (255, 250, 240));

        // 8. Solid Snake Bandana around forehead with knot!
        self.draw_model_box_textured(px, py, pz, -13, -body_h - 1, 4, 26, 4, 10, &rot, Some(TextureId::PlattyBandana), (180, 230, 180));
        let knot_flutter = if (platty.anim_frame / 6) % 2 == 0 { -3 } else { 2 };
        self.draw_model_box_textured(px, py, pz, -15 + knot_flutter, -body_h, -10, 5, 6, 8, &rot, Some(TextureId::PlattyBandana), (170, 210, 170));
    }

    // -------------------------------------------------------------------------
    // ACT 1: SENTRIES & DRONES
    // -------------------------------------------------------------------------

    fn draw_sentry(&self, s: &Sentry, frame: u16) {
        let sx = s.x;
        let sy = s.y;
        let sz = s.z;
        let rot = Mat3I16::rotate_y(s.angle);

        if s.stun_timer > 0 {
            // Knocked out sentry lying flat
            self.draw_model_box(sx, sy, sz, -12, -6, -18, 24, 6, 36, &rot, (50, 65, 80));
            let star_p = scene::project_vertex(Vec3I16::new(sx as i16, -18, sz as i16));
            if star_p.sz > 20 {
                let star_off = ((frame * 4) % 20) as i16 - 10;
                gpu::draw_rect_flat(star_p.sx + star_off, star_p.sy - 8, 4, 4, 255, 240, 60);
            }
        } else {
            // Standing sentry (boots, camo uniform, head, ranger hat, tactical rifle)
            self.draw_model_box(sx, sy, sz, -8, -16, -8, 16, 16, 16, &rot, (45, 52, 60)); // Boots/Legs
            self.draw_model_box_textured(sx, sy, sz, -10, -32, -8, 20, 16, 16, &rot, Some(TextureId::SentryCamo), (170, 170, 170)); // Camo Uniform
            self.draw_model_box(sx, sy, sz, -8, -44, -8, 16, 12, 16, &rot, (210, 175, 140)); // Head
            self.draw_model_box(sx, sy, sz, -12, -48, -12, 24, 6, 24, &rot, (85, 75, 50)); // Ranger Hat
            self.draw_model_box_textured(sx, sy, sz, 8, -26, 4, 6, 6, 22, &rot, Some(TextureId::MetalGrate), (140, 140, 140)); // Tactical Rifle

            if s.see_player {
                // Red "!" Exclamation mark overhead
                let p = scene::project_vertex(Vec3I16::new(sx as i16, -65, sz as i16));
                if p.sz > 20 {
                    gpu::draw_rect_flat(p.sx - 3, p.sy - 16, 6, 12, 250, 30, 30);
                    gpu::draw_rect_flat(p.sx - 3, p.sy - 2, 6, 4, 250, 30, 30);
                    gpu::draw_rect_flat(p.sx - 1, p.sy - 14, 2, 8, 255, 220, 220);
                }
            }
        }
    }

    fn draw_drone(&self, d: &Drone, frame: u8) {
        let rot = Mat3I16::rotate_y(d.angle);
        self.draw_model_box(d.x, d.y, d.z, -10, -10, -10, 20, 20, 20, &rot, (70, 75, 85));
        self.draw_model_box(d.x, d.y, d.z, -4, -4, 10, 8, 8, 4, &rot, (240, 45, 45));

        let p = scene::project_vertex(Vec3I16::new(d.x as i16, (d.y - 12) as i16, d.z as i16));
        if p.sz > 20 {
            let r_off = if (frame / 2) % 2 == 0 { 14 } else { -14 };
            gpu::draw_line_mono(p.sx - r_off, p.sy, p.sx + r_off, p.sy, 220, 230, 240);
        }
    }

    // -------------------------------------------------------------------------
    // ACT 2: YARRA RIVER 5-LANE RUNNER OBSTACLES
    // -------------------------------------------------------------------------

    fn draw_river_obstacle(&self, obs: &RiverObstacle, frame: u8) {
        let rot = Mat3I16::rotate_y(0);
        let ox = obs.x;
        let oy = obs.y;
        let oz = obs.z;

        match obs.kind {
            RiverObstacleType::TreeLog => {
                // Fallen river gum log across lane
                self.draw_model_box(ox, oy, oz, -18, -10, -8, 36, 10, 16, &rot, (110, 65, 35));
                self.draw_model_box(ox, oy, oz, -16, -12, -6, 32, 2, 12, &rot, (75, 120, 50)); // Moss
            }
            RiverObstacleType::LowBranch => {
                // Low-hanging branch (must duck/crawl under)
                self.draw_model_box(ox, oy, oz, -24, -8, -10, 48, 8, 20, &rot, (95, 55, 30));
                self.draw_model_box(ox, oy, oz, -26, -14, -12, 52, 6, 24, &rot, (45, 130, 50)); // Foliage
            }
            RiverObstacleType::TigerSnake => {
                // Wriggling yellow and black striped tiger snake
                let wiggle = if (frame / 6) % 2 == 0 { 3 } else { -3 };
                self.draw_model_box(ox + wiggle, oy, oz, -6, -6, -14, 12, 6, 28, &rot, (230, 200, 30));
                self.draw_model_box(ox - wiggle, oy, oz, -5, -7, -10, 10, 2, 8, &rot, (30, 30, 30));
            }
            RiverObstacleType::GiantSpider => {
                // Giant Australian Huntsman spider on floating bark
                self.draw_model_box(ox, oy, oz, -12, -4, -12, 24, 4, 24, &rot, (130, 85, 45)); // Bark
                self.draw_model_box(ox, oy, oz, -8, -10, -8, 16, 6, 16, &rot, (50, 35, 25));   // Spider
            }
            RiverObstacleType::RiverTuber => {
                // Tourist floating on bright inflatable inner tube
                let bob = if (frame / 8) % 2 == 0 { 2 } else { 0 };
                self.draw_model_box(ox, oy + bob, oz, -16, -10, -16, 32, 10, 32, &rot, (255, 60, 140)); // Pink tube
                self.draw_model_box(ox, oy + bob, oz, -8, -18, -6, 16, 12, 12, &rot, (220, 185, 150));  // Swimmer
            }
            RiverObstacleType::PaddleBoarder => {
                // Stand-up paddle boarder with board and paddle
                self.draw_model_box(ox, oy, oz, -8, -4, -20, 16, 4, 40, &rot, (40, 190, 220)); // SUP board
                self.draw_model_box(ox, oy, oz, -6, -26, -4, 12, 22, 10, &rot, (215, 175, 140)); // Paddler
                self.draw_model_box(ox + 8, oy, oz, 0, -28, -6, 2, 26, 2, &rot, (140, 110, 70)); // Oar
            }
            RiverObstacleType::Swimmer => {
                // Swimmer with swim cap and splashing arms
                let bob = if (frame / 6) % 2 == 0 { 2 } else { -2 };
                self.draw_model_box(ox, oy + bob, oz, -8, -8, -10, 16, 8, 20, &rot, (220, 180, 140));
                self.draw_model_box(ox, oy + bob, oz, -6, -12, 0, 12, 6, 8, &rot, (240, 220, 40)); // Cap
            }
            RiverObstacleType::Koala => {
                // Fuzzy koala clinging to eucalyptus tree on bank
                self.draw_model_box(ox, oy, oz, -8, -26, -8, 16, 26, 16, &rot, (120, 70, 40)); // Trunk
                self.draw_model_box(ox + 6, oy, oz, 0, -22, -6, 12, 14, 12, &rot, (160, 160, 165)); // Koala
                self.draw_model_box(ox + 8, oy, oz, 4, -24, 2, 6, 6, 6, &rot, (30, 30, 30)); // Nose
            }
        }
    }

    // -------------------------------------------------------------------------
    // ACT 3: MELBOURNE CITY FROGGER VEHICLES
    // -------------------------------------------------------------------------

    fn draw_vehicle(&self, v: &CityVehicle) {
        let rot = Mat3I16::rotate_y(if v.vx > 0 { 64 } else { 192 });
        let vx = v.x;
        let vy = 0;
        let vz = v.z;
        let len = v.length;
        let half_len = len / 2;

        match v.kind {
            VehicleType::Taxi => {
                // Melbourne Yellow Taxi
                self.draw_model_box(vx, vy, vz, -12, -12, -half_len, 24, 12, len, &rot, (245, 210, 20));
                self.draw_model_box(vx, vy, vz, -10, -18, -half_len + 8, 20, 6, len - 16, &rot, (40, 60, 80)); // Cabin
                self.draw_model_box(vx, vy, vz, -4, -22, -4, 8, 4, 8, &rot, (255, 255, 255)); // Taxi roof sign
            }
            VehicleType::Sedan => {
                // Blue/Gray Sedan
                self.draw_model_box(vx, vy, vz, -12, -12, -half_len, 24, 12, len, &rot, v.color);
                self.draw_model_box(vx, vy, vz, -10, -18, -half_len + 6, 20, 6, len - 12, &rot, (50, 70, 90));
            }
            VehicleType::Tram => {
                // Classic Green & Gold Melbourne W-Class Tram
                self.draw_model_box(vx, vy, vz, -14, -24, -half_len, 28, 24, len, &rot, (30, 150, 65));
                self.draw_model_box(vx, vy, vz, -15, -12, -half_len, 30, 4, len, &rot, (220, 185, 45)); // Gold stripe
                self.draw_model_box(vx, vy, vz, -13, -20, -half_len + 6, 26, 8, len - 12, &rot, (240, 240, 210)); // Windows
            }
            VehicleType::Truck => {
                // Red Semi-Trailer Freight Truck
                self.draw_model_box(vx, vy, vz, -15, -28, -half_len, 30, 28, len - 18, &rot, (200, 205, 215)); // Cargo container
                self.draw_model_box(vx, vy, vz, -14, -22, half_len - 18, 28, 22, 18, &rot, (220, 45, 40)); // Red cab
            }
            VehicleType::SportsCar => {
                // Fast red sports speedster
                self.draw_model_box(vx, vy, vz, -12, -8, -half_len, 24, 8, len, &rot, (235, 40, 40));
                self.draw_model_box(vx, vy, vz, -10, -12, -half_len + 6, 20, 4, len - 12, &rot, (20, 20, 20));
            }
        }

        // Headlights projected onto road
        let light_dir = if v.vx > 0 { 28 } else { -28 };
        let lp = scene::project_vertex(Vec3I16::new((vx + light_dir) as i16, 0, vz as i16));
        if lp.sz > 20 {
            gpu::draw_rect_flat(lp.sx - 6, lp.sy - 3, 12, 6, 255, 245, 180);
        }
    }

    // -------------------------------------------------------------------------
    // ACT 4: COASTAL BEACH PLATFORMS & CRABS
    // -------------------------------------------------------------------------

    fn draw_beach_platform(&self, p: &BeachPlatform) {
        if p.is_parasol {
            // Bouncing beach parasol umbrella
            let rot = Mat3I16::rotate_y(0);
            let px = p.x + p.w / 2;
            let py = p.y;
            let pz = p.z + p.d / 2;
            self.draw_model_box(px, py, pz, -2, -28, -2, 4, 28, 4, &rot, (200, 200, 200)); // Mast
            // Canopy
            self.draw_model_box_textured(px, py, pz, -18, -32, -18, 36, 6, 36, &rot, Some(TextureId::Parasol), (220, 220, 220)); // Striped parasol
            self.draw_model_box(px, py, pz, -12, -34, -12, 24, 4, 24, &rot, (255, 255, 255));
        } else {
            // Stepped sandstone cliff ledge
            self.draw_box_3d_textured(p.x, p.z, p.w, -p.y, p.d, TextureId::RockCliff, (175, 145, 95));
        }
    }

    fn draw_crab(&self, crab: &BeachCrab, frame: u8) {
        let rot = Mat3I16::rotate_y(if crab.vx > 0 { 64 } else { 192 });
        let cx = crab.x;
        let cy = crab.y;
        let cz = crab.z;

        // Red crab shell
        self.draw_model_box(cx, cy, cz, -10, -8, -8, 20, 8, 16, &rot, (220, 55, 35));
        // Twin snapping claws
        let claw_wiggle = if (frame / 4) % 2 == 0 { 2 } else { -2 };
        self.draw_model_box(cx, cy, cz, -14, -10, 8 + claw_wiggle, 6, 6, 8, &rot, (240, 70, 45));
        self.draw_model_box(cx, cy, cz, 8, -10, 8 - claw_wiggle, 6, 6, 8, &rot, (240, 70, 45));
        // Googly stalk eyes
        self.draw_model_box(cx, cy, cz, -6, -12, 6, 3, 4, 3, &rot, (255, 255, 255));
        self.draw_model_box(cx, cy, cz, 3, -12, 6, 3, 4, 3, &rot, (255, 255, 255));
    }

    // -------------------------------------------------------------------------
    // COLLECTIBLES & PARTICLES
    // -------------------------------------------------------------------------

    fn draw_collectible(&self, c: &Collectible, frame: u8) {
        let bob = if (frame / 6) % 2 == 0 { 2 } else { 0 };
        let p = scene::project_vertex(Vec3I16::new(c.x as i16, (c.y - bob) as i16, c.z as i16));
        if p.sz > 20 {
            match c.kind {
                CollectibleType::YabbyRation => {
                    gpu::draw_rect_flat(p.sx - 6, p.sy - 6, 12, 12, 45, 140, 240);
                    gpu::draw_rect_flat(p.sx - 3, p.sy - 3, 6, 6, 255, 230, 80);
                }
                CollectibleType::ChaffBattery => {
                    gpu::draw_rect_flat(p.sx - 4, p.sy - 7, 8, 14, 60, 220, 90);
                    gpu::draw_rect_flat(p.sx - 2, p.sy - 9, 4, 3, 200, 200, 200);
                }
                CollectibleType::LetterPage => {
                    gpu::draw_rect_flat(p.sx - 7, p.sy - 5, 14, 10, 250, 245, 230);
                    gpu::draw_rect_flat(p.sx - 2, p.sy - 2, 4, 4, 210, 45, 45);
                }
                CollectibleType::BuriedYabby => {
                    let spark = if (frame / 4) % 2 == 0 { 255 } else { 180 };
                    gpu::draw_rect_flat(p.sx - 6, p.sy - 6, 12, 12, spark, spark, 60);
                }
                CollectibleType::StarYabby => {
                    // Golden Star Yabby (Mario 64 star)
                    let spark = if (frame / 3) % 2 == 0 { 255 } else { 200 };
                    gpu::draw_rect_flat(p.sx - 8, p.sy - 8, 16, 16, spark, spark, 40);
                    gpu::draw_rect_flat(p.sx - 4, p.sy - 4, 8, 8, 255, 255, 180);
                }
            }
        }
    }

    fn draw_particles(&self, entities: &EntityManager) {
        for p in entities.particles.iter() {
            if !p.active {
                continue;
            }
            let proj = scene::project_vertex(Vec3I16::new(p.x as i16, p.y as i16, p.z as i16));
            if proj.sz > 20 && proj.sx >= 0 && proj.sx < SCREEN_W && proj.sy >= 0 && proj.sy < SCREEN_H {
                gpu::draw_rect_flat(proj.sx, proj.sy, p.size as u16, p.size as u16, p.color.0, p.color.1, p.color.2);
            }
        }
    }

    // -------------------------------------------------------------------------
    // HUD & STAGE INTERFACES
    // -------------------------------------------------------------------------

    pub fn draw_hud(&self, platty: &Platypus, entities: &EntityManager, act: Act) {
        // TOP-LEFT: LIFE BAR & STAGE LABEL (x: 8, y: 8, w: 100, h: 36)
        gpu::draw_rect_flat(8, 8, 100, 36, 12, 18, 24);
        gpu::draw_rect_flat(10, 10, 96, 32, 4, 8, 12);

        self.font.draw_text(14, 12, "LIFE", (230, 50, 50));
        for i in 0..platty.max_health {
            let lx = 48 + i as i16 * 16;
            if i < platty.health {
                gpu::draw_rect_flat(lx, 13, 12, 8, 235, 45, 45);
                gpu::draw_rect_flat(lx + 1, 14, 10, 2, 255, 180, 180);
            } else {
                gpu::draw_rect_flat(lx, 13, 12, 8, 60, 30, 35);
            }
        }

        if platty.y > 0 || platty.state == PlayerState::Swimming || platty.state == PlayerState::Submerged {
            self.font.draw_text(14, 26, "O2", (70, 210, 255));
            gpu::draw_rect_flat(40, 27, 60, 7, 20, 35, 55);
            let o2_w = (platty.air as u32 * 58 / 100) as u16;
            let o2_col = if platty.air < 30 { (255, 60, 50) } else { (80, 220, 255) };
            gpu::draw_rect_flat(41, 28, o2_w, 5, o2_col.0, o2_col.1, o2_col.2);
        } else {
            let stage_lbl = match act {
                Act::Act1Sanctuary => "STAGE 1",
                Act::Act1Boss => "STAGE 1.3",
                Act::Act2Bushland => "STAGE 2",
                Act::Act3City => "STAGE 3",
                Act::Act4Ocean => "STAGE 4",
            };
            self.font.draw_text(14, 26, stage_lbl, (240, 210, 100));
        }

        // TOP-CENTER: SCORE & YABBIES COUNTER (x: 114, y: 8, w: 96, h: 36)
        gpu::draw_rect_flat(114, 8, 96, 36, 12, 18, 24);
        gpu::draw_rect_flat(116, 10, 92, 32, 4, 8, 12);

        let mut score_str = [b'0'; 6];
        let mut sc = platty.score;
        for i in (0..6).rev() {
            score_str[i] = (sc % 10) as u8 + b'0';
            sc /= 10;
        }
        if let Ok(s) = core::str::from_utf8(&score_str) {
            self.font.draw_text(122, 12, s, (240, 240, 240));
        }

        let mut yabbies_buf = [b'Y', b'A', b'B', b':', b'x', b'0', b'0'];
        yabbies_buf[5] = ((platty.yabbies_collected / 10) % 10) as u8 + b'0';
        yabbies_buf[6] = (platty.yabbies_collected % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&yabbies_buf) {
            self.font.draw_text(122, 26, s, (120, 210, 255));
        }

        // STAGE-SPECIFIC TOP-RIGHT HUD
        match act {
            Act::Act1Sanctuary => {
                // Metal Gear Solid Soliton Radar
                self.draw_soliton_radar(platty, entities);
            }
            Act::Act1Boss => {
                // Searchlight Mech Boss Health Bar
                self.draw_boss_hud(&entities.boss_mech, &entities.power_conduits);
            }
            Act::Act2Bushland => {
                // Yarra River Runner distance HUD
                gpu::draw_rect_flat(218, 8, 94, 36, 12, 28, 18);
                gpu::draw_rect_flat(220, 10, 90, 32, 4, 16, 10);
                self.font.draw_text(224, 12, "YARRA RUN", (100, 255, 140));
                let dist_pct = ((platty.z * 100) / (22 * TILE_SZ)).min(100);
                gpu::draw_rect_flat(224, 28, 80, 6, 20, 45, 25);
                gpu::draw_rect_flat(225, 29, (dist_pct as u32 * 78 / 100) as u16, 4, 80, 230, 120);
            }
            Act::Act3City => {
                // City Frogger Avenues crossed
                gpu::draw_rect_flat(218, 8, 94, 36, 24, 18, 12);
                gpu::draw_rect_flat(220, 10, 90, 32, 14, 8, 6);
                self.font.draw_text(224, 12, "MELBOURNE", (255, 220, 80));
                let progress = ((22 * TILE_SZ - platty.z) * 100 / (20 * TILE_SZ)).clamp(0, 100);
                gpu::draw_rect_flat(224, 28, 80, 6, 45, 25, 20);
                gpu::draw_rect_flat(225, 29, (progress as u32 * 78 / 100) as u16, 4, 255, 180, 40);
            }
            Act::Act4Ocean => {
                // Beach 3D Platformer stars
                gpu::draw_rect_flat(218, 8, 94, 36, 16, 24, 34);
                gpu::draw_rect_flat(220, 10, 90, 32, 8, 14, 20);
                self.font.draw_text(224, 12, "BEACH DUNES", (255, 230, 90));
                self.font.draw_text(224, 26, "GO TO PIP!", (255, 180, 180));
            }
        }
    }

    fn draw_soliton_radar(&self, platty: &Platypus, entities: &EntityManager) {
        let rx: i16 = 236;
        let ry: i16 = 8;
        let rw: u16 = 76;
        let rh: u16 = 76;

        gpu::draw_rect_flat(rx, ry, rw, rh, 10, 35, 25);
        gpu::draw_rect_flat(rx + 2, ry + 2, rw - 4, rh - 4, 4, 18, 12);

        if let AlertState::Alert(timer) = entities.alert_state {
            gpu::draw_rect_flat(rx + 2, ry + 2, rw - 4, 14, 220, 30, 30);
            self.font.draw_text(rx + 4, ry + 3, "ALERT 99.99", (255, 255, 255));
            for i in 0..6 {
                let sx = rx + 4 + ((timer as i16 * 17 + i * 23) % (rw as i16 - 12)).abs();
                let sy = ry + 18 + ((timer as i16 * 11 + i * 31) % (rh as i16 - 24)).abs();
                gpu::draw_rect_flat(sx, sy, 6, 2, 230, 40, 40);
            }
            return;
        }

        let center_x = rx + (rw as i16 / 2);
        let center_y = ry + (rh as i16 / 2);

        // Platty Chevron at center
        gpu::draw_rect_flat(center_x - 2, center_y - 2, 4, 4, 255, 255, 255);
        let fwd_dx = ((sin_1_3_12(platty.angle) as i32 * 6) >> 12) as i16;
        let fwd_dz = ((cos_1_3_12(platty.angle) as i32 * 6) >> 12) as i16;
        gpu::draw_line_mono(center_x, center_y, center_x + fwd_dx, center_y + fwd_dz, 255, 255, 255);

        // Sentry red dots and vision wedges
        for s in entities.sentries.iter() {
            if !s.active {
                continue;
            }
            let dx = ((s.x - platty.x) / 12) as i16;
            let dz = ((s.z - platty.z) / 12) as i16;
            let sx = center_x + dx;
            let sy = center_y + dz;

            if sx >= rx + 4 && sx < rx + rw as i16 - 4 && sy >= ry + 4 && sy < ry + rh as i16 - 4 {
                if s.stun_timer > 0 {
                    gpu::draw_rect_flat(sx - 1, sy - 1, 3, 3, 80, 140, 220);
                } else {
                    gpu::draw_rect_flat(sx - 2, sy - 2, 4, 4, 240, 40, 40);
                    let v_dx = ((sin_1_3_12(s.angle) as i32 * 8) >> 12) as i16;
                    let v_dz = ((cos_1_3_12(s.angle) as i32 * 8) >> 12) as i16;
                    gpu::draw_line_mono(sx, sy, sx + v_dx, sy + v_dz, 240, 60, 60);
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // TITLE, STAGE CLEAR, AND ENDING SCREENS
    // -------------------------------------------------------------------------

    pub fn draw_title_screen(&self, frame: u8) {
        // Draw full-screen title background texture (16-bit direct color at VRAM 640,0)
        let tpage = crate::title_bg::title_bg_tpage();
        let material = psx_gpu::material::TextureMaterial::opaque(0, tpage, (0x80, 0x80, 0x80));
        psx_gpu::draw_sprite_material(0, 0, 320, 240, (0, 0), material);

        self.font.draw_text(60, 40, "PLATTYPUS : TACTICAL ESPIONAGE", (120, 255, 160));
        self.font.draw_text(90, 60, "PROJECT PSOXIDE 3D", (220, 240, 255));

        // Animated Platty emblem box
        gpu::draw_rect_flat(120, 85, 80, 50, 20, 45, 35);
        gpu::draw_rect_flat(122, 87, 76, 46, 120, 75, 40); // Fur
        gpu::draw_rect_flat(155, 95, 35, 14, 65, 62, 60);  // Duck bill
        gpu::draw_rect_flat(115, 85, 90, 8, 30, 210, 75);   // Green bandana

        let blink = (frame / 20) % 2 == 0;
        if blink {
            self.font.draw_text(85, 160, "PRESS START TO INFILTRATE", (255, 230, 80));
        }

        self.font.draw_text(45, 190, "STAGE 1: HEALESVILLE SANCTUARY (STEALTH)", (140, 200, 160));
        self.font.draw_text(45, 202, "STAGE 2: YARRA RIVER RAPIDS (RUNNER)", (140, 200, 160));
        self.font.draw_text(45, 214, "STAGE 3: MELBOURNE DOWNTOWN (FROGGER)", (140, 200, 160));
        self.font.draw_text(45, 226, "STAGE 4: COASTAL DUNES & SURF (3D JUMP)", (140, 200, 160));
    }

    pub fn draw_stage_clear(&self, act: Act, score: u32, yabbies: u16) {
        gpu::draw_rect_flat(0, 0, 320, 240, 8, 24, 16);

        self.font.draw_text(95, 45, "STAGE COMPLETED!", (120, 255, 160));
        self.font.draw_text(55, 75, act.title(), (255, 230, 80));

        let mut sc_buf = [b'S', b'C', b'O', b'R', b'E', b':', b' ', b'0', b'0', b'0', b'0', b'0', b'0', 0];
        let mut s = score;
        for i in (7..13).rev() {
            sc_buf[i] = (s % 10) as u8 + b'0';
            s /= 10;
        }
        if let Ok(st) = core::str::from_utf8(&sc_buf[..13]) {
            self.font.draw_text(110, 115, st, (240, 240, 240));
        }

        let mut yab_buf = [b'Y', b'A', b'B', b'B', b'I', b'E', b'S', b':', b' ', b'x', b'0', b'0', 0];
        yab_buf[10] = ((yabbies / 10) % 10) as u8 + b'0';
        yab_buf[11] = (yabbies % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&yab_buf[..12]) {
            self.font.draw_text(110, 135, st, (120, 210, 255));
        }

        self.font.draw_text(75, 175, "PRESS CROSS FOR NEXT ACT BRIEFING", (255, 255, 255));
    }

    pub fn draw_ending(&self, frame: u8) {
        gpu::draw_rect_flat(0, 0, 320, 240, 30, 80, 140); // Sunset coastal sky
        gpu::draw_rect_flat(0, 150, 320, 90, 220, 190, 130); // Golden sand beach

        self.font.draw_text(70, 30, "MISSION ACCOMPLISHED!", (255, 240, 120));
        self.font.draw_text(50, 50, "WELCOME TO THE WORLD, BABY PIP!", (255, 255, 255));

        // Platty (big brother)
        gpu::draw_rect_flat(80, 130, 48, 28, 145, 95, 48);
        gpu::draw_rect_flat(120, 138, 26, 12, 65, 62, 60); // Bill
        gpu::draw_rect_flat(75, 130, 56, 6, 30, 210, 75);  // Green bandana

        // Baby sister Pip (little golden-brown hatchling platypus!)
        let pip_bounce = if (frame / 12) % 2 == 0 { 2 } else { 0 };
        gpu::draw_rect_flat(175, 142 - pip_bounce, 24, 16, 200, 150, 90);
        gpu::draw_rect_flat(195, 146 - pip_bounce, 14, 8, 80, 75, 70); // Tiny duck bill
        gpu::draw_rect_flat(182, 140 - pip_bounce, 4, 4, 255, 150, 180); // Little pink bow!

        // Golden egg shell fragments
        gpu::draw_rect_flat(165, 150, 10, 8, 255, 240, 180);
        gpu::draw_rect_flat(210, 150, 8, 8, 255, 240, 180);

        self.font.draw_text(60, 185, "BURROW COMMAND: WE'RE SO PROUD!", (100, 255, 160));
        self.font.draw_text(85, 215, "THANK YOU FOR PLAYING!", (255, 255, 255));
    }
}
