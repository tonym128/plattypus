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
    SearchlightMech, Sentry, SentryState, VehicleType,
};
use crate::level::{Act, CellType, Level, GRID_D, GRID_W, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};

use crate::texture::{FaceDirection, TextureAtlasManager, TextureId, gouraud_face_colors};

use psx_font::{fonts::BASIC, FontAtlas};
use psx_gpu::{
    self as gpu,
    framebuf::FrameBuffer,
    material::BlendMode,
    Resolution,
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
    pub costume: u8,
    pub wireframe: bool,
    pub language: u8,
    pub video_mode: psx_gpu::VideoMode,
    pub screen_offset_x: i8,
    pub screen_offset_y: i8,
}

impl Renderer {
    pub fn new() -> Self {
        let (detected_mode, _) = crate::save::detect_console_region();
        gpu::init(detected_mode, Resolution::R320X240);
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
            costume: 0,
            wireframe: false,
            language: 0,
            video_mode: detected_mode,
            screen_offset_x: 0,
            screen_offset_y: 0,
        }
    }

    pub fn apply_display_offset(&self) {
        gpu::set_display_offset(
            self.video_mode,
            Resolution::R320X240,
            self.screen_offset_x as i16,
            self.screen_offset_y as i16,
        );
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
        match level.act.chapter() {
            1 => gpu::draw_rect_flat(0, 0, 320, 240, 10, 14, 20), // Dark military compound
            2 => gpu::draw_rect_flat(0, 0, 320, 240, 16, 40, 24),  // Yarra forest canopy
            3 => gpu::draw_rect_flat(0, 0, 320, 240, 14, 16, 28),      // Melbourne night sky
            _ => gpu::draw_rect_flat(0, 0, 320, 240, 50, 130, 210),   // Coastal ocean sky
        }

        // Draw ground searchlights & vision cones on floor (Blended)
        if level.act == Act::Act1_3MechBoss {
            self.draw_boss_searchlights(&entities.boss_mech);
        } else {
            self.draw_vision_cones(entities);
        }

        // Row-based depth sorting (far to near) to eliminate clipping
        let min_gx = ((self.cam_x - 320) / TILE_SZ).clamp(0, GRID_W as i32) as usize;
        let max_gx = ((self.cam_x + 320) / TILE_SZ + 1).clamp(0, GRID_W as i32) as usize;
        let min_gz = ((self.cam_z + 80) / TILE_SZ).clamp(0, GRID_D as i32) as usize;
        let max_gz = ((self.cam_z + 680) / TILE_SZ + 1).clamp(0, GRID_D as i32) as usize;

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

            // 2. Draw entities situated in this row (with horizontal frustum culling)
            for s in entities.sentries.iter() {
                if s.active && (s.x - self.cam_x).abs() < 340 && s.z >= row_z_min && s.z < row_z_max {
                    self.draw_sentry(s, entities.frame);
                }
            }
            for d in entities.drones.iter() {
                if d.active && (d.x - self.cam_x).abs() < 340 && d.z >= row_z_min && d.z < row_z_max {
                    self.draw_drone(d, frame);
                }
            }
            for c in entities.power_conduits.iter() {
                if c.active && (c.x - self.cam_x).abs() < 340 && c.z >= row_z_min && c.z < row_z_max {
                    self.draw_power_conduit(c, frame);
                }
            }
            let mech = &entities.boss_mech;
            if mech.active && mech.z >= row_z_min && mech.z < row_z_max {
                self.draw_searchlight_mech(mech, frame);
            }
            let jetski = &entities.boss_jetski;
            if jetski.active && jetski.z >= row_z_min && jetski.z < row_z_max {
                self.draw_jetski_boss(jetski, frame);
            }
            let sniper = &entities.boss_sniper;
            if sniper.active && sniper.z >= row_z_min && sniper.z < row_z_max {
                self.draw_sniper_kookaburra(sniper, frame);
            }
            let exc = &entities.boss_excavator;
            if exc.active && exc.z >= row_z_min && exc.z < row_z_max {
                self.draw_excavator_boss(exc, frame);
            }
            for obs in entities.river_obstacles.iter() {
                if obs.active && (obs.x - self.cam_x).abs() < 340 && obs.z >= row_z_min && obs.z < row_z_max {
                    self.draw_river_obstacle(obs, frame);
                }
            }
            for v in entities.vehicles.iter() {
                if v.active && (v.x - self.cam_x).abs() < 340 && v.z >= row_z_min && v.z < row_z_max {
                    self.draw_vehicle(v);
                }
            }
            for p in entities.beach_platforms.iter() {
                if p.active && (p.x - self.cam_x).abs() < 340 && p.z >= row_z_min && p.z < row_z_max {
                    self.draw_beach_platform(p);
                }
            }
            for crab in entities.beach_crabs.iter() {
                if crab.active && (crab.x - self.cam_x).abs() < 340 && crab.z >= row_z_min && crab.z < row_z_max {
                    self.draw_crab(crab, frame);
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
        if level.act == Act::Act1_3MechBoss && entities.boss_mech.shockwave_active {
            self.draw_shockwave_ring(&entities.boss_mech);
        }

        // Draw 3D Particles
        self.draw_particles(entities);

        // Draw Electro-Reception Sonar Overlay (wall-penetrating blips)
        if platty.electro_timer > 0 {
            self.draw_electro_sonar_overlay(platty, entities, level, frame);
        }
    }

    fn draw_electro_sonar_overlay(
        &self,
        platty: &Platypus,
        entities: &EntityManager,
        level: &Level,
        frame: u8,
    ) {
        // 1. Concentric Sonar Pulse Wave expanding across ground
        let wave_progress = (180 - platty.electro_timer) as i32;
        let wave_rad = (wave_progress * 4) % 200;
        let mut prev_p: Option<(i16, i16, u16)> = None;
        for step in 0..=12 {
            let ang = (step * 21) as u16;
            let px = platty.x + ((cos_1_3_12(ang) as i32 * wave_rad) >> 12);
            let pz = platty.z + ((sin_1_3_12(ang) as i32 * wave_rad) >> 12);
            let cur_p = scene::project_vertex(Vec3I16::new(px as i16, 0, pz as i16));

            if let Some((prev_sx, prev_sy, prev_sz)) = prev_p {
                if cur_p.sz > 20 && prev_sz > 20 {
                    gpu::draw_line_mono(prev_sx, prev_sy, cur_p.sx, cur_p.sy, 60, 230, 255);
                }
            }
            prev_p = Some((cur_p.sx, cur_p.sy, cur_p.sz));
        }

        // 2. Wall-Penetrating Sentries with tactical brackets and pulsing heartbeats
        for s in entities.sentries.iter() {
            if !s.active {
                continue;
            }
            let p = scene::project_vertex(Vec3I16::new(s.x as i16, -22, s.z as i16));
            if p.sz > 20 && p.sx >= 10 && p.sx < SCREEN_W - 10 && p.sy >= 10 && p.sy < SCREEN_H - 10 {
                // Tactical cyan brackets '[' and ']' around sentry silhouette
                gpu::draw_rect_flat(p.sx - 12, p.sy - 16, 2, 32, 40, 240, 255);
                gpu::draw_rect_flat(p.sx + 10, p.sy - 16, 2, 32, 40, 240, 255);
                gpu::draw_rect_flat(p.sx - 12, p.sy - 16, 6, 2, 40, 240, 255);
                gpu::draw_rect_flat(p.sx + 6, p.sy - 16, 6, 2, 40, 240, 255);
                gpu::draw_rect_flat(p.sx - 12, p.sy + 14, 6, 2, 40, 240, 255);
                gpu::draw_rect_flat(p.sx + 6, p.sy + 14, 6, 2, 40, 240, 255);

                // Pulsing red/white heartbeat dot at sentry chest
                let pulse = if (frame / 3) % 2 == 0 { (255, 50, 50) } else { (255, 200, 200) };
                gpu::draw_rect_flat(p.sx - 2, p.sy - 4, 4, 4, pulse.0, pulse.1, pulse.2);

                self.font.draw_text(p.sx - 16, p.sy - 26, "SENTRY", (60, 230, 255));
            }
        }

        // 3. Wall-Penetrating Drones
        for d in entities.drones.iter() {
            if !d.active {
                continue;
            }
            let p = scene::project_vertex(Vec3I16::new(d.x as i16, d.y as i16, d.z as i16));
            if p.sz > 20 && p.sx >= 10 && p.sx < SCREEN_W - 10 && p.sy >= 10 && p.sy < SCREEN_H - 10 {
                gpu::draw_rect_flat(p.sx - 8, p.sy - 8, 16, 2, 60, 230, 255);
                gpu::draw_rect_flat(p.sx - 8, p.sy + 6, 16, 2, 60, 230, 255);
                gpu::draw_rect_flat(p.sx - 8, p.sy - 8, 2, 16, 60, 230, 255);
                gpu::draw_rect_flat(p.sx + 6, p.sy - 8, 2, 16, 60, 230, 255);
                self.font.draw_text(p.sx - 14, p.sy - 18, "DRONE", (60, 230, 255));
            }
        }

        // 4. Wall-Penetrating Collectibles & Buried Yabbies
        for c in entities.collectibles.iter() {
            if !c.active {
                continue;
            }
            let p = scene::project_vertex(Vec3I16::new(c.x as i16, (c.y - 8) as i16, c.z as i16));
            if p.sz > 20 && p.sx >= 10 && p.sx < SCREEN_W - 10 && p.sy >= 10 && p.sy < SCREEN_H - 10 {
                let spark = if (frame / 2) % 2 == 0 { 255 } else { 180 };
                gpu::draw_rect_flat(p.sx - 4, p.sy - 4, 8, 8, spark, spark, 60);
                gpu::draw_rect_flat(p.sx - 2, p.sy - 2, 4, 4, 255, 255, 220);

                let label = match c.kind {
                    CollectibleType::CardboardBox => "BOX",
                    CollectibleType::LetterPage => "INTEL",
                    _ => "YABBY",
                };
                self.font.draw_text(p.sx - 10, p.sy - 14, label, (255, 230, 80));
            }
        }

        // 5. Wall-Penetrating Ventilation Shafts (AirDucts)
        let min_gx = ((self.cam_x - 320) / TILE_SZ).max(0) as usize;
        let max_gx = ((self.cam_x + 320) / TILE_SZ + 1).min(GRID_W as i32) as usize;
        let min_gz = ((self.cam_z + 80) / TILE_SZ).max(0) as usize;
        let max_gz = ((self.cam_z + 680) / TILE_SZ + 1).min(GRID_D as i32) as usize;

        for gz in min_gz..max_gz {
            for gx in min_gx..max_gx {
                if level.get_cell(gx, gz) == CellType::AirDuct {
                    let wx = (gx as i32) * TILE_SZ + 32;
                    let wz = (gz as i32) * TILE_SZ + 32;
                    let p = scene::project_vertex(Vec3I16::new(wx as i16, -14, wz as i16));
                    if p.sz > 20 && p.sx >= 10 && p.sx < SCREEN_W - 10 && p.sy >= 10 && p.sy < SCREEN_H - 10 {
                        gpu::draw_rect_flat(p.sx - 10, p.sy - 10, 20, 2, 80, 240, 255);
                        gpu::draw_rect_flat(p.sx - 10, p.sy + 8, 20, 2, 80, 240, 255);
                        gpu::draw_rect_flat(p.sx - 10, p.sy - 10, 2, 20, 80, 240, 255);
                        gpu::draw_rect_flat(p.sx + 8, p.sy - 10, 2, 20, 80, 240, 255);
                        self.font.draw_text(p.sx - 10, p.sy - 18, "VENT", (80, 240, 255));
                    }
                }
            }
        }
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
        let (floor_r, floor_g, floor_b) = match act.chapter() {
            1 => (30, 38, 44),   // Dark tarmac / concrete
            2 => (25, 75, 40),   // Lush Yarra riverbank moss & grass
            3 => (48, 52, 58),   // City asphalt road
            _ => (220, 200, 145), // Warm coastal sand
        };

        let gx = (wx / TILE_SZ) as usize;
        let gz = (wz / TILE_SZ) as usize;

        match cell {
            CellType::Floor => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                match act.chapter() {
                    3 => {
                        // Road dividing dashed white lines
                        if gz == 18 || gz == 12 || gz == 6 {
                            self.draw_box_3d(wx + 24, wz + 28, 16, 1, 8, (230, 230, 230));
                        }
                        // Sidewalk curbs and streetlamps on pedestrian boundaries
                        if (gx == 4 || gx == 19) && (gz % 4 == 0) {
                            self.draw_streetlamp(wx, wz);
                        }
                    }
                    4 => {
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
                // Subtle dark ground shadow / ambient occlusion footprint beneath crate
                let (sr, sg, sb) = (floor_r / 2, floor_g / 2, floor_b / 2);
                Self::draw_quad_3d(
                    Vec3I16::new((wx + 6) as i16, 0, (wz + 6) as i16),
                    Vec3I16::new((wx + 58) as i16, 0, (wz + 6) as i16),
                    Vec3I16::new((wx + 6) as i16, 0, (wz + 60) as i16),
                    Vec3I16::new((wx + 58) as i16, 0, (wz + 60) as i16),
                    sr, sg, sb,
                );
                // 48x48x48 cargo crate with texture & Gouraud shading
                self.draw_box_3d_textured(wx + 8, wz + 8, 48, 48, 48, TextureId::Crate, (180, 180, 180));
            }
            CellType::Container => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                let (sr, sg, sb) = (floor_r / 2, floor_g / 2, floor_b / 2);
                Self::draw_quad_3d(
                    Vec3I16::new(wx as i16, 0, wz as i16),
                    Vec3I16::new((wx + 64) as i16, 0, wz as i16),
                    Vec3I16::new(wx as i16, 0, (wz + 64) as i16),
                    Vec3I16::new((wx + 64) as i16, 0, (wz + 64) as i16),
                    sr, sg, sb,
                );
                let (c_tex, c_col, h) = match act.chapter() {
                    1 => (TextureId::ConcreteWall, (150, 150, 150), 70),
                    2 => (TextureId::RiverLog, (140, 140, 140), 64),
                    3 => (TextureId::CityBrick, (160, 160, 160), 110), // Giant illuminated skyscraper!
                    _ => (TextureId::RockCliff, (160, 160, 160), 55), // Stepped sandcliff
                };
                self.draw_box_3d_textured(wx + 2, wz + 2, 60, h, 60, c_tex, c_col);

                // Add glowing windows and beacons on city skyscrapers
                if act.chapter() == 3 {
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
                match act.chapter() {
                    2 => {
                        // Bushland riverbank: mossy bank + gum tree
                        self.draw_box_3d_textured(wx, wz, 64, 32, 64, TextureId::GumLeaves, (140, 140, 140));
                        if (gx + gz) % 2 == 0 {
                            self.draw_gum_tree(wx, wz);
                        }
                    }
                    1 => {
                        self.draw_box_3d_textured(wx, wz, 64, 80, 64, TextureId::ConcreteWall, (150, 150, 150));
                    }
                    3 => {
                        self.draw_box_3d_textured(wx, wz, 64, 80, 64, TextureId::CityBrick, (160, 160, 160));
                    }
                    _ => {
                        self.draw_box_3d_textured(wx, wz, 64, 60, 64, TextureId::RockCliff, (160, 160, 160));
                    }
                }
            }
            CellType::AirDuct => {
                self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b, act);
                let (sr, sg, sb) = (floor_r / 2, floor_g / 2, floor_b / 2);
                Self::draw_quad_3d(
                    Vec3I16::new((wx + 6) as i16, 0, (wz + 6) as i16),
                    Vec3I16::new((wx + 58) as i16, 0, (wz + 6) as i16),
                    Vec3I16::new((wx + 6) as i16, 0, (wz + 60) as i16),
                    Vec3I16::new((wx + 58) as i16, 0, (wz + 60) as i16),
                    sr, sg, sb,
                );
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
            CellType::MetalGrate => {
                self.draw_floor_tile(wx, wz, 90, 95, 105, act);
                // Heavy steel bevel framing on catwalk boundaries
                self.draw_box_3d(wx, wz, 64, 2, 4, (120, 130, 140));
                self.draw_box_3d(wx, wz + 60, 64, 2, 4, (120, 130, 140));
            }
        }
    }

    fn draw_floor_tile(&self, wx: i32, wz: i32, r: u8, g: u8, b: u8, act: Act) {
        let v0 = Vec3I16::new(wx as i16, 0, wz as i16);
        let v1 = Vec3I16::new((wx + TILE_SZ) as i16, 0, wz as i16);
        let v2 = Vec3I16::new(wx as i16, 0, (wz + TILE_SZ) as i16);
        let v3 = Vec3I16::new((wx + TILE_SZ) as i16, 0, (wz + TILE_SZ) as i16);

        let floor_tex = match act.chapter() {
            1 => TextureId::MetalGrate,
            2 => TextureId::GumLeaves,
            3 => TextureId::CityAsphalt,
            _ => TextureId::BeachSand,
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

        let tint = match act.chapter() {
            4 => (160, 200, 240),
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
        if is_current || act.chapter() == 4 {
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

        if self.wireframe {
            let mut proj = [scene::project_vertex(world_pts[0]); 8];
            for i in 1..8 {
                proj[i] = scene::project_vertex(world_pts[i]);
            }
            const EDGES: [(usize, usize); 12] = [
                (0, 1), (1, 3), (3, 2), (2, 0),
                (4, 5), (5, 7), (7, 6), (6, 4),
                (0, 4), (1, 5), (2, 6), (3, 7),
            ];
            let lr = col.0.max(40);
            let lg = col.1.max(220);
            let lb = col.2.max(80);
            for &(a, b) in &EDGES {
                if proj[a].sz >= 20 && proj[b].sz >= 20 {
                    gpu::draw_line_mono(proj[a].sx, proj[a].sy, proj[b].sx, proj[b].sy, lr, lg, lb);
                }
            }
            return;
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

    fn draw_jetski_boss(&self, jetski: &crate::entities::JetSkiBoss, _frame: u8) {
        if !jetski.active {
            return;
        }
        let rot = Mat3I16::rotate_y(128); // Facing south toward Platty
        let jx = jetski.x;
        let jy = jetski.y;
        let jz = jetski.z;

        let col = if jetski.hit_timer > 0 && (jetski.hit_timer / 2) % 2 == 1 {
            (255, 255, 255)
        } else if jetski.is_stalled {
            (255, 160, 40)
        } else {
            (35, 75, 160) // Police watercraft blue
        };

        // Hull
        self.draw_model_box(jx, jy, jz, -14, -6, -24, 28, 8, 48, &rot, col);
        // Deck / Seat
        self.draw_model_box(jx, jy, jz, -8, -14, -10, 16, 8, 26, &rot, (220, 220, 230));
        // Windshield
        self.draw_model_box(jx, jy, jz, -10, -18, 6, 20, 6, 4, &rot, (80, 210, 255));
        // Ranger Driver
        self.draw_model_box(jx, jy, jz, -6, -26, 0, 12, 12, 10, &rot, (130, 90, 50));
        self.draw_model_box(jx, jy, jz, -8, -30, -2, 16, 4, 14, &rot, (100, 70, 40));
        // Outboard motor at rear
        self.draw_model_box(jx, jy, jz, -6, -10, -28, 12, 12, 8, &rot, (40, 45, 50));
    }

    fn draw_sniper_kookaburra(&self, sniper: &crate::entities::SniperBoss, frame: u8) {
        if !sniper.active {
            return;
        }
        let rot = Mat3I16::rotate_y(0);
        let sx = sniper.x;
        let sy = sniper.y;
        let sz = sniper.z;

        let body_col = if sniper.hit_timer > 0 && (sniper.hit_timer / 2) % 2 == 1 {
            (255, 255, 255)
        } else {
            (190, 175, 150) // Feathered gray-brown
        };

        // Kookaburra Body
        self.draw_model_box(sx, sy, sz, -10, -18, -10, 20, 18, 20, &rot, body_col);
        // Wings
        self.draw_model_box(sx, sy, sz, -14, -16, -8, 4, 14, 16, &rot, (90, 65, 45));
        self.draw_model_box(sx, sy, sz, 10, -16, -8, 4, 14, 16, &rot, (90, 65, 45));
        // Large Laughing Beak
        self.draw_model_box(sx, sy, sz, -4, -12, 10, 8, 6, 12, &rot, (230, 150, 40));
        // Sniper Rifle Barrel
        self.draw_model_box(sx, sy, sz, 4, -14, 4, 3, 3, 24, &rot, (40, 40, 45));

        // Red Laser Targeting line from rifle to ground
        if !sniper.is_vulnerable {
            let muzzle = scene::project_vertex(Vec3I16::new((sx + 5) as i16, (sy - 13) as i16, (sz + 28) as i16));
            let target = scene::project_vertex(Vec3I16::new(sniper.laser_x as i16, 0, sniper.laser_z as i16));
            if muzzle.sz > 20 && target.sz > 20 {
                let flash = if (frame / 2) % 2 == 0 { 255 } else { 160 };
                gpu::draw_line_mono(muzzle.sx, muzzle.sy, target.sx, target.sy, flash, 30, 30);
                gpu::draw_rect_flat(target.sx - 2, target.sy - 2, 5, 5, 255, 40, 40);
            }
        }
    }

    fn draw_excavator_boss(&self, exc: &crate::entities::ExcavatorBoss, _frame: u8) {
        if !exc.active {
            return;
        }
        let rot = Mat3I16::rotate_y(0);
        let ex = exc.x;
        let ey = exc.y;
        let ez = exc.z;

        let yellow = if exc.hit_timer > 0 && (exc.hit_timer / 2) % 2 == 1 {
            (255, 255, 255)
        } else {
            (230, 185, 30) // Caterpillar construction yellow
        };

        // Left & Right Caterpillar Treads
        self.draw_model_box(ex, ey, ez, -34, -12, -28, 14, 12, 56, &rot, (40, 42, 45));
        self.draw_model_box(ex, ey, ez, 20, -12, -28, 14, 12, 56, &rot, (40, 42, 45));
        // Main Engine Chassis
        self.draw_model_box(ex, ey, ez, -20, -28, -22, 40, 18, 44, &rot, yellow);
        // Cabin Cockpit Glass
        self.draw_model_box(ex, ey, ez, -16, -42, 0, 18, 14, 18, &rot, (90, 210, 255));
        // Dr. Cane Toad inside cockpit
        self.draw_model_box(ex, ey, ez, -12, -38, 4, 10, 10, 10, &rot, (60, 160, 50));
        // Rear Engine Exhaust / Radiator
        self.draw_model_box(ex, ey, ez, -12, -34, -26, 24, 12, 6, &rot, (240, 80, 40));

        // Articulated Hydraulic Crane Arm
        let arm_reach = (exc.claw_angle as i32 * 40) / 180;
        self.draw_model_box(ex, ey, ez, 4, -38, 12, 8, 8, 30, &rot, yellow);
        self.draw_model_box(ex, ey, ez, 4 + arm_reach / 2, -30, 36, 8, 8, 24, &rot, (150, 150, 160));
        // Heavy Shovel Bucket Claw
        self.draw_model_box(ex, ey, ez, 2 + arm_reach, -18, 54, 14, 16, 16, &rot, (60, 65, 70));
    }

    fn draw_jetski_boss_hud(&self, jetski: &crate::entities::JetSkiBoss) {
        let bx: i16 = 180;
        let by: i16 = 8;
        let bw: u16 = 134;
        let bh: u16 = 36;

        gpu::draw_rect_flat(bx, by, bw, bh, 12, 24, 32);
        gpu::draw_rect_flat(bx + 2, by + 2, bw - 4, bh - 4, 4, 10, 16);

        self.font.draw_text(bx + 6, by + 4, "RANGER JET SKI", (80, 220, 255));
        if jetski.is_stalled {
            self.font.draw_text(bx + 6, by + 18, "STALLED! HIT!", (255, 230, 40));
        } else {
            self.font.draw_text(bx + 6, by + 18, "ENGINE HP", (240, 80, 80));
            let hp_w = (jetski.health as u16 * 18).min(54);
            gpu::draw_rect_flat(bx + 72, by + 19, hp_w, 8, 255, 60, 60);
        }
    }

    fn draw_sniper_boss_hud(&self, sniper: &crate::entities::SniperBoss) {
        let bx: i16 = 180;
        let by: i16 = 8;
        let bw: u16 = 134;
        let bh: u16 = 36;

        gpu::draw_rect_flat(bx, by, bw, bh, 28, 16, 12);
        gpu::draw_rect_flat(bx + 2, by + 2, bw - 4, bh - 4, 12, 6, 4);

        self.font.draw_text(bx + 6, by + 4, "SNIPER KOOKY", (255, 120, 40));
        if sniper.is_vulnerable {
            self.font.draw_text(bx + 6, by + 18, "JAMMED! STRIKE!", (255, 240, 80));
        } else {
            self.font.draw_text(bx + 6, by + 18, "TARGET LOCK", (255, 60, 60));
            let aim_w = ((sniper.aim_timer.min(100) as u32 * 48) / 100) as u16;
            gpu::draw_rect_flat(bx + 80, by + 19, aim_w, 8, 255, 40, 40);
        }
    }

    fn draw_excavator_boss_hud(&self, exc: &crate::entities::ExcavatorBoss) {
        let bx: i16 = 180;
        let by: i16 = 8;
        let bw: u16 = 134;
        let bh: u16 = 36;

        gpu::draw_rect_flat(bx, by, bw, bh, 24, 20, 10);
        gpu::draw_rect_flat(bx + 2, by + 2, bw - 4, bh - 4, 10, 8, 4);

        self.font.draw_text(bx + 6, by + 4, "DR. TOAD-DOZER", (255, 200, 40));
        self.font.draw_text(bx + 6, by + 18, "VALVES", (140, 230, 80));
        let hp_w = (exc.health as u16 * 14).min(56);
        gpu::draw_rect_flat(bx + 66, by + 19, hp_w, 8, 255, 180, 40);
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

        // Cardboard Box Disguise ("The Bill Box")
        if platty.in_box {
            let shadow_p = scene::project_vertex(Vec3I16::new(px as i16, 0, pz as i16));
            if shadow_p.sz > 20 {
                gpu::draw_rect_flat(shadow_p.sx - 14, shadow_p.sy - 4, 28, 8, 20, 24, 30);
            }

            let is_moving = platty.vx != 0 || platty.vz != 0;
            let waddle = if is_moving && ((platty.anim_frame / 4) % 2 == 1) { 1 } else { 0 };

            // 1. The Cardboard Box outer shell (28 wide x 20 high x 32 deep)
            self.draw_model_box_textured(px, py, pz, -14, -20, -16, 28, 20, 32, &rot, Some(TextureId::Crate), (185, 145, 95));

            // 2. Cut-out Eye Holes on front face
            self.draw_model_box(px, py, pz, -8, -14, 16, 4, 3, 2, &rot, (30, 25, 20));
            self.draw_model_box(px, py, pz, 4, -14, 16, 4, 3, 2, &rot, (30, 25, 20));

            // 3. Duck Bill poking out slightly through bottom slit (+Z)
            self.draw_model_box_textured(px, py, pz, -5, -4, 15, 10, 4, 6, &rot, Some(TextureId::PlattyBill), (150, 150, 150));

            // 4. Little orange webbed feet peeking out when shuffling
            if is_moving {
                self.draw_model_box(px, py, pz, -15, -2, -2 + waddle * 4, 4, 2, 6, &rot, (215, 130, 45));
                self.draw_model_box(px, py, pz, 11, -2, -2 - waddle * 4, 4, 2, 6, &rot, (215, 130, 45));
            }

            // 5. Caution Tape Stripe on top
            self.draw_model_box_textured(px, py, pz, -10, -21, -6, 20, 1, 12, &rot, Some(TextureId::CautionStripe), (220, 200, 80));
            return;
        }

        // Soft ground shadow beneath Platty
        let shadow_p = scene::project_vertex(Vec3I16::new(px as i16, 0, pz as i16));
        if shadow_p.sz > 20 {
            gpu::draw_rect_flat(shadow_p.sx - 8, shadow_p.sy - 3, 16, 6, 20, 24, 30);
        }

        let is_crawl = platty.state == PlayerState::BellyCrawl;
        let body_h = if is_crawl { 8 } else { 16 };

        // Determine costume coloring & accessories
        let (fur_col, under_col, bill_col, tail_col, feet_col, satchel_col, bandana_col, is_tuxedo) = match self.costume {
            1 => (
                (28, 28, 34),     // Tuxedo jacket black
                (245, 245, 250),  // Crisp white shirt bib
                (140, 140, 140),  // Slate gray bill
                (40, 40, 48),     // Black paddle tail
                (20, 20, 25),     // Shiny black dress shoes
                (35, 35, 42),     // Elegant black leather satchel
                (220, 30, 30),    // Red silk bow tie
                true,
            ),
            2 => (
                (40, 180, 170),   // Stealth Camo active shimmer
                (60, 220, 200),
                (100, 240, 230),
                (30, 150, 140),
                (60, 200, 190),
                (50, 190, 180),
                (140, 255, 240),
                false,
            ),
            _ => (
                (160, 110, 60),   // Classic brown fur
                (200, 150, 100),  // Underbelly cream
                (150, 150, 150),  // Leathery duck bill
                (130, 80, 45),    // Beaver paddle tail
                (215, 130, 45),   // Webbed feet
                (185, 135, 65),   // Brown leather satchel
                (180, 230, 180),  // Solid Snake bandana
                false,
            ),
        };

        // 1. Platty Body
        self.draw_model_box_textured(px, py, pz, -12, -body_h, -14, 24, body_h, 28, &rot, Some(TextureId::PlattyFur), fur_col);

        // 2. Underbelly Bib
        self.draw_model_box_textured(px, py, pz, -8, -body_h / 2, -10, 16, body_h / 2, 20, &rot, Some(TextureId::PlattyFur), under_col);

        // 3. Duck Bill (+Z in local facing) with leathery sensory texture
        self.draw_model_box_textured(px, py, pz, -7, -8, 14, 14, 5, 16, &rot, Some(TextureId::PlattyBill), bill_col);

        // 4. Beaver Paddle Tail (-Z in local facing)
        let tail_wobble = ((platty.anim_frame / 4) % 2) as i32 * 2;
        self.draw_model_box_textured(px, py, pz, -9 + tail_wobble, -5, -30, 18, 5, 18, &rot, Some(TextureId::PlattyFur), tail_col);

        // 5. Webbed Feet
        self.draw_model_box(px, py, pz, -15, -3, 6, 5, 3, 8, &rot, feet_col);
        self.draw_model_box(px, py, pz, 10, -3, 6, 5, 3, 8, &rot, feet_col);
        self.draw_model_box(px, py, pz, -15, -3, -12, 5, 3, 8, &rot, feet_col);
        self.draw_model_box(px, py, pz, 10, -3, -12, 5, 3, 8, &rot, feet_col);

        // Glowing golden venom spurs extended on rear ankles during SpurStrike CQC
        if platty.state == PlayerState::SpurStrike {
            self.draw_model_box(px, py, pz, -17, -7, -14, 4, 7, 7, &rot, (255, 235, 50));
            self.draw_model_box(px, py, pz, 13, -7, -14, 4, 7, 7, &rot, (255, 235, 50));
        }

        // 6. Expressive Eyes
        self.draw_model_box(px, py, pz, -8, -body_h - 2, 8, 4, 4, 4, &rot, (255, 255, 255));
        self.draw_model_box(px, py, pz, -8, -body_h - 1, 11, 2, 2, 2, &rot, (20, 20, 20));
        self.draw_model_box(px, py, pz, 4, -body_h - 2, 8, 4, 4, 4, &rot, (255, 255, 255));
        self.draw_model_box(px, py, pz, 6, -body_h - 1, 11, 2, 2, 2, &rot, (20, 20, 20));

        // 7. Leather Satchel with Important Letter!
        self.draw_model_box(px, py, pz, 5, -body_h - 3, -4, 9, 8, 10, &rot, satchel_col);
        self.draw_model_box(px, py, pz, 7, -body_h - 5, -2, 5, 3, 6, &rot, (255, 250, 240));

        // 8. Head accessory: Red Bowtie (Tuxedo) or Snake Bandana
        if is_tuxedo {
            // Sharp red silk bowtie at collar
            self.draw_model_box(px, py, pz, -4, -body_h / 2 - 2, 12, 8, 4, 3, &rot, bandana_col);
            self.draw_model_box(px, py, pz, -2, -body_h / 2 - 1, 14, 4, 2, 2, &rot, (255, 80, 80));
        } else {
            // Solid Snake Bandana around forehead with fluttering knot
            self.draw_model_box_textured(px, py, pz, -13, -body_h - 1, 4, 26, 4, 10, &rot, Some(TextureId::PlattyBandana), bandana_col);
            let knot_flutter = if (platty.anim_frame / 6) % 2 == 0 { -3 } else { 2 };
            self.draw_model_box_textured(px, py, pz, -15 + knot_flutter, -body_h, -10, 5, 6, 8, &rot, Some(TextureId::PlattyBandana), bandana_col);
        }
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

            // 3 Orbiting Golden Daze Stars circling overhead
            let base_ang = (frame as u16) * 5;
            for i in 0..3 {
                let star_ang = base_ang.wrapping_add((i as u16) * 85);
                let off_x = ((cos_1_3_12(star_ang) as i32 * 16) >> 12) as i16;
                let off_z = ((sin_1_3_12(star_ang) as i32 * 16) >> 12) as i16;
                let bob_y = (((frame as i32 + (i as i32 * 6)) % 12) - 6).abs() as i16;

                let star_p = scene::project_vertex(Vec3I16::new(
                    (sx as i16) + off_x,
                    -16 - bob_y,
                    (sz as i16) + off_z,
                ));
                if star_p.sz > 20 {
                    // Golden star diamond with bright white core
                    gpu::draw_rect_flat(star_p.sx - 2, star_p.sy - 2, 5, 5, 255, 230, 40);
                    gpu::draw_rect_flat(star_p.sx - 1, star_p.sy - 1, 3, 3, 255, 255, 200);
                }
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
            } else if s.state == SentryState::Investigating {
                // Yellow "?" Question mark overhead
                let p = scene::project_vertex(Vec3I16::new(sx as i16, -65, sz as i16));
                if p.sz > 20 {
                    gpu::draw_rect_flat(p.sx - 4, p.sy - 18, 8, 3, 255, 225, 40);
                    gpu::draw_rect_flat(p.sx + 2, p.sy - 15, 3, 6, 255, 225, 40);
                    gpu::draw_rect_flat(p.sx - 2, p.sy - 10, 6, 3, 255, 225, 40);
                    gpu::draw_rect_flat(p.sx - 1, p.sy - 7, 3, 3, 255, 225, 40);
                    gpu::draw_rect_flat(p.sx - 1, p.sy - 2, 3, 3, 255, 225, 40);
                    gpu::draw_rect_flat(p.sx - 3, p.sy - 17, 3, 1, 255, 255, 220);
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
                CollectibleType::CardboardBox => {
                    // Miniature Cardboard Box with caution tape
                    let rot = Mat3I16::rotate_y(((frame as u16) * 3) % 256);
                    self.draw_model_box_textured(c.x, c.y - bob, c.z, -8, -12, -8, 16, 12, 16, &rot, Some(TextureId::Crate), (185, 145, 95));
                    self.draw_model_box(c.x, c.y - bob, c.z, -9, -8, -9, 18, 3, 18, &rot, (240, 200, 40));
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

    pub fn draw_hud(&self, platty: &Platypus, entities: &EntityManager, level: &Level, frame: u8) {
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
            self.font.draw_text(14, 26, level.act.stage_label(), (240, 210, 100));
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

        // Box Inventory / Equipped Status Badge
        if platty.has_box {
            gpu::draw_rect_flat(8, 48, 100, 14, 12, 18, 24);
            gpu::draw_rect_flat(10, 50, 96, 10, 4, 8, 12);
            let (txt, col) = if platty.in_box {
                ("BOX: ACTIVE", (100, 240, 120))
            } else {
                ("L1: EQUIP BOX", (180, 180, 180))
            };
            self.font.draw_text(14, 50, txt, col);
        }

        // Electro Charge Meter or Active Sonar Banner
        if platty.electro_charge > 0 {
            gpu::draw_rect_flat(114, 48, 96, 14, 12, 18, 24);
            gpu::draw_rect_flat(116, 50, 92, 10, 4, 8, 12);
            let chg_w = ((platty.electro_charge as u32 * 90) / 30) as u16;
            gpu::draw_rect_flat(117, 51, chg_w, 8, 60, 220, 255);
            self.font.draw_text(126, 50, "SONAR CHG", (255, 255, 255));
        } else if platty.electro_timer > 0 {
            gpu::draw_rect_flat(114, 48, 96, 14, 12, 18, 24);
            gpu::draw_rect_flat(116, 50, 92, 10, 4, 8, 12);
            self.font.draw_text(120, 50, "SONAR ACTIVE", (80, 240, 255));
        }

        // STAGE-SPECIFIC TOP-RIGHT HUD
        match level.act {
            Act::Act1_1Drainage | Act::Act1_2Barracks | Act::Act3_2Laneways => {
                // Metal Gear Solid Soliton Radar for stealth/urban infiltration
                self.draw_soliton_radar(platty, entities, level, frame);
            }
            Act::Act1_3MechBoss => {
                // Searchlight Mech Boss Health Bar
                self.draw_boss_hud(&entities.boss_mech, &entities.power_conduits);
            }
            Act::Act2_1Rapids => {
                // Yarra River Runner distance HUD
                gpu::draw_rect_flat(218, 8, 94, 36, 12, 28, 18);
                gpu::draw_rect_flat(220, 10, 90, 32, 4, 16, 10);
                self.font.draw_text(224, 12, "YARRA RUN", (100, 255, 140));
                let dist_pct = ((platty.z * 100) / (22 * TILE_SZ)).min(100);
                gpu::draw_rect_flat(224, 28, 80, 6, 20, 45, 25);
                gpu::draw_rect_flat(225, 29, (dist_pct as u32 * 78 / 100) as u16, 4, 80, 230, 120);
            }
            Act::Act2_2Mangroves => {
                gpu::draw_rect_flat(218, 8, 94, 36, 10, 26, 20);
                gpu::draw_rect_flat(220, 10, 90, 32, 4, 14, 10);
                self.font.draw_text(224, 12, "MANGROVES", (80, 240, 180));
                self.font.draw_text(224, 26, "SONAR TO NAV", (120, 220, 255));
            }
            Act::Act2_3JetSkiBoss => {
                self.draw_jetski_boss_hud(&entities.boss_jetski);
            }
            Act::Act3_1Highway => {
                // City Frogger Avenues crossed
                gpu::draw_rect_flat(218, 8, 94, 36, 24, 18, 12);
                gpu::draw_rect_flat(220, 10, 90, 32, 14, 8, 6);
                self.font.draw_text(224, 12, "MELBOURNE", (255, 220, 80));
                let progress = ((22 * TILE_SZ - platty.z) * 100 / (20 * TILE_SZ)).clamp(0, 100);
                gpu::draw_rect_flat(224, 28, 80, 6, 45, 25, 20);
                gpu::draw_rect_flat(225, 29, (progress as u32 * 78 / 100) as u16, 4, 255, 180, 40);
            }
            Act::Act3_3SniperBoss => {
                self.draw_sniper_boss_hud(&entities.boss_sniper);
            }
            Act::Act4_1Dunes => {
                // Beach 3D Platformer
                gpu::draw_rect_flat(218, 8, 94, 36, 16, 24, 34);
                gpu::draw_rect_flat(220, 10, 90, 32, 8, 14, 20);
                self.font.draw_text(224, 12, "BEACH DUNES", (255, 230, 90));
                self.font.draw_text(224, 26, "GO TO PIER!", (255, 180, 180));
            }
            Act::Act4_2PierTrench => {
                gpu::draw_rect_flat(218, 8, 94, 36, 12, 20, 36);
                gpu::draw_rect_flat(220, 10, 90, 32, 6, 10, 22);
                self.font.draw_text(224, 12, "PIER TRENCH", (90, 200, 255));
                self.font.draw_text(224, 26, "SUBMERGE!", (255, 120, 120));
            }
            Act::Act4_3ExcavatorBoss => {
                self.draw_excavator_boss_hud(&entities.boss_excavator);
            }
            Act::VrSneaking | Act::VrCqc | Act::VrSonar | Act::VrSpeed => {
                self.draw_soliton_radar(platty, entities, level, frame);
            }
        }

        // TACTICAL CONTEXTUAL ABILITY PROMPT
        self.draw_context_prompt(platty, entities, level, frame);
    }

    fn draw_context_prompt(&self, platty: &Platypus, entities: &EntityManager, level: &Level, frame: u8) {
        let mut prompt: Option<&'static str> = None;

        let gx = (platty.x / TILE_SZ).clamp(0, (GRID_W - 1) as i32) as usize;
        let gz = (platty.z / TILE_SZ).clamp(0, (GRID_D - 1) as i32) as usize;
        let fwd_dx = (sin_1_3_12(platty.angle) as i32 * 48) >> 12;
        let fwd_dz = (cos_1_3_12(platty.angle) as i32 * 48) >> 12;
        let fgx = ((platty.x + fwd_dx) / TILE_SZ).clamp(0, (GRID_W - 1) as i32) as usize;
        let fgz = ((platty.z + fwd_dz) / TILE_SZ).clamp(0, (GRID_D - 1) as i32) as usize;

        let curr_cell = level.get_cell(gx, gz);
        let fwd_cell = level.get_cell(fgx, fgz);

        // 1. Air duct / crawl vent
        if (curr_cell == CellType::AirDuct || fwd_cell == CellType::AirDuct) && platty.state != PlayerState::BellyCrawl {
            prompt = Some("CIRCLE: CRAWL INTO VENT");
        } else if fwd_cell == CellType::LaserTripwire && platty.state != PlayerState::BellyCrawl {
            // 2. Laser tripwire crawl under
            prompt = Some("CIRCLE: CRAWL UNDER LASER");
        } else if platty.state == PlayerState::Swimming {
            // 3. Water swimming -> submerge
            prompt = Some("CROSS: SUBMERGE DIVE");
        } else if platty.state == PlayerState::Submerged {
            // 4. Submerged -> sonar pulse
            if platty.electro_timer == 0 && platty.electro_charge == 0 {
                prompt = Some("TRIANGLE: SONAR PULSE");
            }
        } else {
            // 5. CQC Takedown behind unalerted sentry
            for s in entities.sentries.iter() {
                if !s.active || s.stun_timer > 0 {
                    continue;
                }
                let dx = s.x - platty.x;
                let dz = s.z - platty.z;
                if dx * dx + dz * dz < 54 * 54 {
                    prompt = Some("SQUARE: CQC TAKEDOWN");
                    break;
                }
            }

            // 6. Cardboard Box disguise near guards
            if prompt.is_none() && platty.has_box && !platty.in_box && matches!(platty.state, PlayerState::Standing | PlayerState::Sneaking | PlayerState::Running) {
                for s in entities.sentries.iter() {
                    if !s.active || s.stun_timer > 0 {
                        continue;
                    }
                    let dx = s.x - platty.x;
                    let dz = s.z - platty.z;
                    if dx * dx + dz * dz < 130 * 130 {
                        prompt = Some("L1: CARDBOARD BOX");
                        break;
                    }
                }
            }
        }

        if let Some(msg) = prompt {
            let msg_len = msg.len() as i16;
            let bw = (msg_len * 8) + 16;
            let bx = (SCREEN_W - bw) / 2;
            let by: i16 = 216;

            let border_col = (20, 60, 48);
            let bg_col = (6, 18, 14);
            gpu::draw_rect_flat(bx, by, bw as u16, 16, border_col.0, border_col.1, border_col.2);
            gpu::draw_rect_flat(bx + 1, by + 1, (bw - 2) as u16, 14, bg_col.0, bg_col.1, bg_col.2);

            let txt_col = if (frame / 8) % 2 == 0 {
                (255, 235, 80)
            } else {
                (180, 255, 140)
            };
            self.font.draw_text(bx + 8, by + 4, msg, txt_col);
        }
    }

    fn draw_soliton_radar(&self, platty: &Platypus, entities: &EntityManager, level: &Level, frame: u8) {
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

        // Exit Burrow / Objective Marker (blinking yellow diamond / chevron)
        if level.exit_x > 0 || level.exit_z > 0 {
            let obj_dx = ((level.exit_x - platty.x) / 12) as i16;
            let obj_dz = ((level.exit_z - platty.z) / 12) as i16;
            let target_x = center_x + obj_dx;
            let target_y = center_y + obj_dz;

            let min_x = rx + 5;
            let max_x = rx + rw as i16 - 6;
            let min_y = ry + 5;
            let max_y = ry + rh as i16 - 6;

            let is_outside = target_x < min_x || target_x > max_x || target_y < min_y || target_y > max_y;

            let (mx, my) = if is_outside {
                // Project vector to radar boundary
                let abs_x = obj_dx.abs();
                let abs_z = obj_dz.abs();
                let max_abs = abs_x.max(abs_z);
                if max_abs > 0 {
                    let edge_r = (rw as i16 / 2) - 6; // 32 pixels
                    let ex = (center_x + (obj_dx * edge_r) / max_abs).clamp(min_x, max_x);
                    let ey = (center_y + (obj_dz * edge_r) / max_abs).clamp(min_y, max_y);
                    (ex, ey)
                } else {
                    (center_x, center_y)
                }
            } else {
                (target_x, target_y)
            };

            // Blinking beacon (flashes every 8 frames)
            if (frame / 8) % 2 == 0 {
                if is_outside {
                    // Off-radar chevron indicator (amber/yellow square)
                    gpu::draw_rect_flat(mx - 2, my - 2, 4, 4, 255, 230, 40);
                    gpu::draw_rect_flat(mx - 1, my - 1, 2, 2, 255, 255, 200);
                } else {
                    // On-radar burrow target (yellow diamond with center pip)
                    gpu::draw_rect_flat(mx - 2, my - 2, 5, 5, 255, 230, 40);
                    gpu::draw_rect_flat(mx - 1, my - 1, 3, 3, 20, 50, 30);
                    gpu::draw_rect_flat(mx, my, 1, 1, 255, 255, 180);
                }
            }
        }

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

    pub fn draw_title_screen(
        &self,
        frame: u8,
        selected_menu: usize,
        save_data: &crate::save::SaveData,
    ) {
        // Draw full-screen title background texture (16-bit direct color at VRAM 640,0)
        let tpage = crate::title_bg::title_bg_tpage();
        let material = psx_gpu::material::TextureMaterial::opaque(0, tpage, (0x80, 0x80, 0x80));
        psx_gpu::draw_sprite_material(0, 0, 320, 240, (0, 0), material);

        self.font.draw_text(60, 32, "PLATTYPUS : TACTICAL ESPIONAGE", (120, 255, 160));
        self.font.draw_text(90, 48, "PROJECT PSOXIDE 3D", (220, 240, 255));

        // Animated Platty emblem box
        gpu::draw_rect_flat(120, 68, 80, 46, 20, 45, 35);
        gpu::draw_rect_flat(122, 70, 76, 42, 120, 75, 40); // Fur
        gpu::draw_rect_flat(155, 78, 35, 14, 65, 62, 60);  // Duck bill
        gpu::draw_rect_flat(115, 68, 90, 8, 30, 210, 75);   // Green bandana

        let has_save = save_data.unlocked_act > 0;
        let act = Act::from_u8(save_data.unlocked_act);
        let stage_label = act.stage_label();

        if has_save {
            // Tactical 4-Item Mission Selection Menu Box (with Continue)
            gpu::draw_rect_flat(38, 118, 244, 94, 12, 20, 30);
            gpu::draw_rect_flat(40, 120, 240, 90, 6, 10, 16);

            let (cont_text, new_text, vr_text, opt_text) = match self.language {
                1 => ("CONTINUER", "NOUVELLE CAMPAGNE", "SIMULATEUR ENTRAINEMENT VR", "OPTIONS ET EQUIPEMENT"),
                2 => ("FORTSETZEN", "NEUE KAMPAGNE", "VR-TRAININGSSIMULATOR", "OPTIONEN UND AUSRUESTUNG"),
                3 => ("CONTINUAR", "NUEVA CAMPANA", "SIMULADOR ENTRENAMIENTO VR", "OPCIONES Y EQUIPO"),
                4 => ("SAIKAI", "SHINKI SAKUSEN", "VR KUNREN SIMULATOR", "TOKUSHU SOUBI (OPTIONS)"),
                _ => ("CONTINUE", "NEW CAMPAIGN", "VR TRAINING SIMULATOR", "SPECIAL OPTIONS & GEAR"),
            };

            let menu_items = [cont_text, new_text, vr_text, opt_text];

            for (i, label) in menu_items.iter().enumerate() {
                let y = 126 + (i as i16 * 21);
                if i == selected_menu {
                    gpu::draw_rect_flat(46, y - 3, 228, 17, 25, 75, 50);
                    let pulse = if (frame / 12) % 2 == 0 { (255, 240, 100) } else { (180, 255, 140) };
                    self.font.draw_text(50, y, ">", pulse);
                    self.font.draw_text(62, y, label, (255, 255, 255));
                    if i == 0 {
                        self.font.draw_text(178, y, stage_label, (255, 235, 80));
                    }
                } else {
                    self.font.draw_text(62, y, label, (130, 160, 175));
                    if i == 0 {
                        self.font.draw_text(178, y, stage_label, (180, 200, 140));
                    }
                }
            }
        } else {
            // Tactical 3-Item Mission Selection Menu Box (Standard)
            gpu::draw_rect_flat(45, 126, 230, 76, 12, 20, 30);
            gpu::draw_rect_flat(47, 128, 226, 72, 6, 10, 16);

            let menu_items = match self.language {
                1 => [
                    "INFILTRATION CAMPAGNE",
                    "SIMULATEUR ENTRAINEMENT VR",
                    "OPTIONS ET EQUIPEMENT",
                ],
                2 => [
                    "KAMPAGNEN-INFILTRATION",
                    "VR-TRAININGSSIMULATOR",
                    "OPTIONEN UND AUSRUESTUNG",
                ],
                3 => [
                    "INFILTRACION DE CAMPANA",
                    "SIMULADOR ENTRENAMIENTO VR",
                    "OPCIONES Y EQUIPO",
                ],
                4 => [
                    "SAKUSEN SENNYUU (CAMPAIGN)",
                    "VR KUNREN SIMULATOR",
                    "TOKUSHU SOUBI (OPTIONS)",
                ],
                _ => [
                    "CAMPAIGN INFILTRATION",
                    "VR TRAINING SIMULATOR",
                    "SPECIAL OPTIONS & GEAR",
                ],
            };

            for (i, label) in menu_items.iter().enumerate() {
                let y = 136 + (i as i16 * 20);
                if i == selected_menu {
                    gpu::draw_rect_flat(52, y - 3, 216, 16, 25, 75, 50);
                    let pulse = if (frame / 12) % 2 == 0 { (255, 240, 100) } else { (180, 255, 140) };
                    self.font.draw_text(56, y, ">", pulse);
                    self.font.draw_text(70, y, label, (255, 255, 255));
                } else {
                    self.font.draw_text(70, y, label, (130, 160, 175));
                }
            }
        }

        let footer_text = match self.language {
            1 => "CROIX / START: CONFIRMER",
            2 => "KREUZ / START: BESTAETIGEN",
            3 => "CRUZ / START: CONFIRMAR",
            4 => "CROSS / START: KETTEI",
            _ => "DPAD: SELECT  |  START / CROSS: CONFIRM",
        };
        self.font.draw_text(48, 216, footer_text, (100, 150, 160));
    }

    pub fn draw_vr_menu(&self, selected_vr: usize, vr_cleared: u8) {
        // Cyber VR matrix background
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 14, 22);

        // Cyber grid lines
        for y in (0..240).step_by(30) {
            gpu::draw_line_mono(0, y, 319, y, 14, 30, 45);
        }
        for x in (0..320).step_by(40) {
            gpu::draw_line_mono(x, 0, x, 239, 14, 30, 45);
        }

        // Header
        gpu::draw_rect_flat(20, 12, 280, 28, 12, 35, 50);
        gpu::draw_rect_flat(22, 14, 276, 24, 6, 20, 32);
        self.font.draw_text(32, 20, "BURROW HQ - VR TRAINING SIMULATOR", (100, 240, 255));

        // 4 VR Simulations
        let vr_names = [
            ("VR-01: SNEAKING BASICS", "INFILTRATE EXIT PAD UNDETECTED"),
            ("VR-02: VENOM SPUR CQC", "NEUTRALIZE 3 SENTRIES WITH CQC"),
            ("VR-03: ELECTRO-SONAR", "PITCH-BLACK RECON USING SONAR"),
            ("VR-04: RAPID SPEED HURDLES", "SPRINT ELEVATED BRIDGE TO EXIT"),
        ];

        for i in 0..4 {
            let y = 48 + (i as i16 * 34);
            let cleared = (vr_cleared & (1 << i)) != 0;
            let is_sel = i == selected_vr;

            if is_sel {
                gpu::draw_rect_flat(20, y, 280, 30, 20, 70, 60);
                gpu::draw_rect_flat(22, y + 2, 276, 26, 10, 35, 30);
                self.font.draw_text(26, y + 6, ">", (255, 235, 80));
            } else {
                gpu::draw_rect_flat(20, y, 280, 30, 10, 20, 30);
                gpu::draw_rect_flat(22, y + 2, 276, 26, 6, 12, 20);
            }

            let name_col = if is_sel { (255, 255, 255) } else { (160, 190, 200) };
            self.font.draw_text(38, y + 5, vr_names[i].0, name_col);

            let sub_col = if is_sel { (140, 220, 180) } else { (100, 130, 140) };
            self.font.draw_text(38, y + 17, vr_names[i].1, sub_col);

            if cleared {
                self.font.draw_text(224, y + 9, "[CLEARED]", (100, 255, 140));
            } else {
                self.font.draw_text(236, y + 9, "[OPEN]", (255, 200, 80));
            }
        }

        // Footer hint
        gpu::draw_rect_flat(20, 192, 280, 36, 10, 18, 28);
        self.font.draw_text(28, 198, "CROSS: ENGAGE SIMULATION", (255, 230, 80));
        self.font.draw_text(28, 212, "CIRCLE: RETURN TO TITLE", (160, 180, 200));

        let cleared_count = (vr_cleared & 1) + ((vr_cleared >> 1) & 1) + ((vr_cleared >> 2) & 1) + ((vr_cleared >> 3) & 1);
        if cleared_count == 4 {
            self.font.draw_text(188, 205, "ALL SIMS 100%!", (120, 255, 160));
        }
    }

    pub fn draw_options_menu(&self, save_data: &crate::save::SaveData, selected_opt: usize) {
        // Tactical dark steel background
        gpu::draw_rect_flat(0, 0, 320, 240, 10, 14, 20);

        // Header
        gpu::draw_rect_flat(20, 8, 280, 24, 25, 35, 45);
        gpu::draw_rect_flat(22, 10, 276, 20, 10, 16, 22);
        self.font.draw_text(34, 14, "SYSTEM CONFIGURATION & GEAR", (255, 230, 80));

        let opt_labels = [
            "COSTUME / TENUE",
            "1994 RETRO WIREFRAME",
            "LANGUAGE / LANGUE",
            "VIDEO STANDARD",
            "SCREEN V-CENTER",
        ];

        let costume_str = match save_data.selected_costume {
            1 => "TUXEDO (CLASSIC BOND)",
            2 => "STEALTH CAMO (SHIMMER)",
            _ => "SNEAKING SUIT (DEFAULT)",
        };

        let wire_str = if save_data.wireframe_enabled != 0 { "< ENABLED >" } else { "< DISABLED >" };

        let lang_str = match save_data.language {
            1 => "< FRANCAIS >",
            2 => "< DEUTSCH >",
            3 => "< ESPANOL >",
            4 => "< NIHONGO (ROMAJI) >",
            _ => "< ENGLISH >",
        };

        let video_str = match save_data.pal_mode {
            0 => "< NTSC 60Hz (FORCE) >",
            1 => "< PAL 50Hz (FORCE) >",
            _ => "< AUTO DETECT (BIOS) >",
        };

        let opt_values = [
            costume_str,
            wire_str,
            lang_str,
            video_str,
            "",
        ];

        for i in 0..5 {
            let y = 35 + (i as i16 * 26);
            let is_sel = selected_opt == i;
            let bg = if is_sel { (20, 60, 48) } else { (12, 18, 24) };
            gpu::draw_rect_flat(20, y, 280, 24, bg.0, bg.1, bg.2);
            gpu::draw_rect_flat(22, y + 2, 276, 20, bg.0 / 2, bg.1 / 2, bg.2 / 2);

            let cursor = if is_sel { ">" } else { " " };
            self.font.draw_text(24, y + 5, cursor, (255, 235, 80));
            self.font.draw_text(34, y + 5, opt_labels[i], if is_sel { (255, 255, 255) } else { (160, 180, 190) });

            if i == 4 {
                let off_y = save_data.screen_offset_y;
                let mut off_buf = [b'<', b' ', b'0', b'0', b' ', b'L', b'N', b' ', b'>', 0];
                let is_neg = off_y < 0;
                let abs_val = off_y.unsigned_abs();
                off_buf[2] = if is_neg { b'-' } else { b'+' };
                off_buf[3] = (abs_val / 10) as u8 + b'0';
                off_buf[4] = (abs_val % 10) as u8 + b'0';
                if let Ok(st) = core::str::from_utf8(&off_buf[..9]) {
                    self.font.draw_text(190, y + 5, st, (100, 230, 255));
                }
            } else {
                self.font.draw_text(168, y + 5, opt_values[i], (120, 255, 160));
            }
        }

        // Hardware Status & Memory Card Info Banner
        let (_, region_name) = crate::save::detect_console_region();
        gpu::draw_rect_flat(20, 168, 280, 18, 10, 16, 26);
        self.font.draw_text(24, 172, "HARDWARE:", (140, 180, 220));
        self.font.draw_text(80, 172, region_name, (255, 230, 80));
        self.font.draw_text(224, 172, "CARD: 1 BLK", (100, 255, 140));

        // Description box
        gpu::draw_rect_flat(20, 188, 280, 48, 8, 12, 16);
        match selected_opt {
            0 => {
                self.font.draw_text(26, 192, "Tuxedo: Beat campaign. Camo: Rank S.", (180, 200, 220));
                self.font.draw_text(26, 205, "DPAD LEFT/RIGHT: Switch costume", (255, 230, 80));
            }
            1 => {
                self.font.draw_text(26, 192, "Experience Plattypus in early 90s PS1 vectors.", (180, 200, 220));
                self.font.draw_text(26, 205, "DPAD LEFT/RIGHT: Toggle Wireframe", (255, 230, 80));
            }
            2 => {
                self.font.draw_text(26, 192, "Select menu language / Choisir la langue.", (180, 200, 220));
                self.font.draw_text(26, 205, "DPAD LEFT/RIGHT: Change Language", (255, 230, 80));
            }
            3 => {
                self.font.draw_text(26, 192, "Switch between 60Hz NTSC and 50Hz PAL modes.", (180, 200, 220));
                self.font.draw_text(26, 205, "DPAD LEFT/RIGHT: Toggle Video Mode", (255, 230, 80));
            }
            _ => {
                self.font.draw_text(26, 192, "Adjust vertical display centering on CRT.", (180, 200, 220));
                self.font.draw_text(26, 205, "DPAD LEFT/RIGHT: Shift Scanlines", (255, 230, 80));
            }
        }
        self.font.draw_text(26, 222, "CIRCLE: RETURN TO TITLE SCREEN", (130, 180, 210));
    }

    pub fn draw_debriefing_screen(
        &self,
        time_s: u32,
        alerts: u16,
        takedowns: u16,
        damage: u16,
        yabbies: u16,
        codename: crate::save::Codename,
    ) {
        // Deep stealth military debriefing background
        gpu::draw_rect_flat(0, 0, 320, 240, 8, 14, 20);

        // Header banner
        gpu::draw_rect_flat(20, 12, 280, 28, 18, 30, 42);
        gpu::draw_rect_flat(22, 14, 276, 24, 6, 12, 18);
        self.font.draw_text(28, 20, "OPERATION DUCK-BILL : MISSION DEBRIEFING", (120, 255, 160));

        // Stats card box
        gpu::draw_rect_flat(20, 46, 280, 92, 14, 22, 30);
        gpu::draw_rect_flat(22, 48, 276, 88, 8, 14, 20);

        // Time format MM:SS
        let mins = time_s / 60;
        let secs = time_s % 60;
        let mut time_str = [b'T', b'I', b'M', b'E', b':', b' ', b'0', b'0', b':', b'0', b'0', 0];
        time_str[6] = ((mins / 10) % 10) as u8 + b'0';
        time_str[7] = (mins % 10) as u8 + b'0';
        time_str[9] = ((secs / 10) % 10) as u8 + b'0';
        time_str[10] = (secs % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&time_str[..11]) {
            self.font.draw_text(30, 54, st, (240, 240, 240));
        }

        // Alerts count
        let mut alert_str = [b'A', b'L', b'E', b'R', b'T', b' ', b'P', b'H', b'A', b'S', b'E', b'S', b':', b' ', b'0', b'0', 0];
        alert_str[14] = ((alerts / 10) % 10) as u8 + b'0';
        alert_str[15] = (alerts % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&alert_str[..16]) {
            self.font.draw_text(30, 68, st, (255, 120, 100));
        }

        // Takedowns count
        let mut cqc_str = [b'C', b'Q', b'C', b' ', b'T', b'A', b'K', b'E', b'D', b'O', b'W', b'N', b'S', b':', b' ', b'0', b'0', 0];
        cqc_str[15] = ((takedowns / 10) % 10) as u8 + b'0';
        cqc_str[16] = (takedowns % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&cqc_str[..17]) {
            self.font.draw_text(30, 82, st, (255, 230, 80));
        }

        // Damage Sustained
        let mut dmg_str = [b'D', b'A', b'M', b'A', b'G', b'E', b' ', b'T', b'A', b'K', b'E', b'N', b':', b' ', b'0', b'0', b' ', b'H', b'P', 0];
        dmg_str[14] = ((damage / 10) % 10) as u8 + b'0';
        dmg_str[15] = (damage % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&dmg_str[..19]) {
            self.font.draw_text(30, 96, st, (255, 160, 140));
        }

        // Yabbies
        let mut yab_str = [b'Y', b'A', b'B', b'B', b'I', b'E', b'S', b' ', b'C', b'A', b'C', b'H', b'E', b':', b' ', b'x', b'0', b'0', 0];
        yab_str[16] = ((yabbies / 10) % 10) as u8 + b'0';
        yab_str[17] = (yabbies % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&yab_str[..18]) {
            self.font.draw_text(30, 110, st, (100, 220, 255));
        }

        // Rank / Codename Insignia Box
        gpu::draw_rect_flat(20, 144, 280, 56, 30, 45, 60);
        gpu::draw_rect_flat(22, 146, 276, 52, 12, 18, 26);

        self.font.draw_text(30, 152, "FINAL OPERATIVE EVALUATION:", (255, 230, 80));
        self.font.draw_text(30, 166, codename.name(), (255, 255, 255));
        self.font.draw_text(30, 178, codename.title(), (120, 255, 160));

        self.font.draw_text(45, 214, "PRESS CROSS TO PROCEED TO EPILOGUE", (255, 255, 255));
    }

    pub fn draw_stage_clear(
        &self,
        act: Act,
        score: u32,
        yabbies: u16,
        time_s: u32,
        alerts: u16,
        takedowns: u16,
    ) {
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 16, 14);

        self.font.draw_text(95, 20, "STAGE COMPLETED!", (120, 255, 160));
        self.font.draw_text(40, 36, act.title(), (255, 230, 80));

        // Tactical Mission Performance Card
        let card_x: i16 = 24;
        let card_y: i16 = 54;
        let card_w: u16 = 272;
        let card_h: u16 = 136;
        gpu::draw_rect_flat(card_x, card_y, card_w, card_h, 18, 54, 40);
        gpu::draw_rect_flat(card_x + 2, card_y + 2, card_w - 4, card_h - 4, 8, 20, 16);

        // Header bar in card
        gpu::draw_rect_flat(card_x + 4, card_y + 4, card_w - 8, 16, 14, 40, 30);
        self.font.draw_text(card_x + 24, card_y + 8, "TACTICAL INFILTRATION REPORT", (140, 240, 200));

        // 1. Stage Time: MM:SS
        let mins = (time_s / 60).min(99);
        let secs = time_s % 60;
        let mut time_buf = *b"STAGE TIME       : 00:00";
        time_buf[19] = ((mins / 10) % 10) as u8 + b'0';
        time_buf[20] = (mins % 10) as u8 + b'0';
        time_buf[22] = ((secs / 10) % 10) as u8 + b'0';
        time_buf[23] = (secs % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&time_buf) {
            self.font.draw_text(card_x + 16, card_y + 28, st, (240, 240, 240));
        }

        // 2. Alerts Triggered
        let mut alert_buf = *b"ALERTS TRIGGERED : 000";
        let a_clamped = alerts.min(999);
        alert_buf[19] = ((a_clamped / 100) % 10) as u8 + b'0';
        alert_buf[20] = ((a_clamped / 10) % 10) as u8 + b'0';
        alert_buf[21] = (a_clamped % 10) as u8 + b'0';
        let alert_col = if alerts == 0 { (100, 255, 140) } else { (255, 120, 100) };
        if let Ok(st) = core::str::from_utf8(&alert_buf) {
            self.font.draw_text(card_x + 16, card_y + 46, st, alert_col);
        }

        // 3. CQC Takedowns
        let mut take_buf = *b"CQC TAKEDOWNS    : 000";
        let t_clamped = takedowns.min(999);
        take_buf[19] = ((t_clamped / 100) % 10) as u8 + b'0';
        take_buf[20] = ((t_clamped / 10) % 10) as u8 + b'0';
        take_buf[21] = (t_clamped % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&take_buf) {
            self.font.draw_text(card_x + 16, card_y + 64, st, (255, 230, 80));
        }

        // 4. Yabbies Recovered
        let mut yab_buf = *b"YABBIES SECURED  : x00";
        yab_buf[20] = ((yabbies / 10) % 10) as u8 + b'0';
        yab_buf[21] = (yabbies % 10) as u8 + b'0';
        if let Ok(st) = core::str::from_utf8(&yab_buf) {
            self.font.draw_text(card_x + 16, card_y + 82, st, (120, 210, 255));
        }

        // 5. Total Score
        let mut sc_buf = *b"OPERATION SCORE  : 000000";
        let mut s = score;
        for i in (19..25).rev() {
            sc_buf[i] = (s % 10) as u8 + b'0';
            s /= 10;
        }
        if let Ok(st) = core::str::from_utf8(&sc_buf) {
            self.font.draw_text(card_x + 16, card_y + 100, st, (255, 255, 255));
        }

        self.font.draw_text(45, 204, "PRESS CROSS FOR NEXT ACT BRIEFING", (255, 255, 255));
    }

    pub fn draw_ending(&self, frame: u8, codename: Option<crate::save::Codename>) {
        gpu::draw_rect_flat(0, 0, 320, 240, 30, 80, 140); // Sunset coastal sky
        gpu::draw_rect_flat(0, 142, 320, 98, 220, 190, 130); // Golden sand beach

        self.font.draw_text(70, 14, "MISSION ACCOMPLISHED!", (255, 240, 120));
        self.font.draw_text(50, 28, "WELCOME TO THE WORLD, BABY PIP!", (255, 255, 255));

        if let Some(c) = codename {
            gpu::draw_rect_flat(30, 44, 260, 34, 15, 30, 45);
            gpu::draw_rect_flat(32, 46, 256, 30, 10, 18, 28);
            self.font.draw_text(40, 49, "OPERATIVE RANK:", (180, 220, 240));
            self.font.draw_text(152, 49, c.name(), (255, 235, 80));
            self.font.draw_text(40, 62, c.title(), (120, 255, 160));
        }

        // Platty (big brother)
        gpu::draw_rect_flat(80, 118, 48, 28, 145, 95, 48);
        gpu::draw_rect_flat(120, 126, 26, 12, 65, 62, 60); // Bill
        gpu::draw_rect_flat(75, 118, 56, 6, 30, 210, 75);  // Green bandana

        // Baby sister Pip (little golden-brown hatchling platypus!)
        let pip_bounce = if (frame / 12) % 2 == 0 { 2 } else { 0 };
        gpu::draw_rect_flat(175, 130 - pip_bounce, 24, 16, 200, 150, 90);
        gpu::draw_rect_flat(195, 134 - pip_bounce, 14, 8, 80, 75, 70); // Tiny duck bill
        gpu::draw_rect_flat(182, 128 - pip_bounce, 4, 4, 255, 150, 180); // Little pink bow!

        // Golden egg shell fragments
        gpu::draw_rect_flat(165, 138, 10, 8, 255, 240, 180);
        gpu::draw_rect_flat(210, 138, 8, 8, 255, 240, 180);

        self.font.draw_text(60, 182, "BURROW COMMAND: WE'RE SO PROUD!", (100, 255, 160));
        self.font.draw_text(85, 208, "THANK YOU FOR PLAYING!", (255, 255, 255));
    }

    pub fn draw_cinematic_letterbox(&self) {
        // Top 24px letterbox bar
        gpu::draw_rect_flat(0, 0, 320, 24, 0, 0, 0);
        gpu::draw_rect_flat(0, 24, 320, 1, 30, 120, 60);

        // Bottom 24px letterbox bar
        gpu::draw_rect_flat(0, 216, 320, 24, 0, 0, 0);
        gpu::draw_rect_flat(0, 215, 320, 1, 30, 120, 60);
    }

    pub fn draw_boss_title_card(&self, act: Act, timer: u16) {
        self.draw_cinematic_letterbox();

        let (name, codename, specs) = match act {
            Act::Act1_3MechBoss => (
                "PERIMETER WALKER MK-I",
                "SEARCHLIGHT MECH",
                "DUAL MEGA-SEARCHLIGHTS & SHOCKWAVE",
            ),
            Act::Act2_3JetSkiBoss => (
                "RANGER CHIEF DAVE",
                "PATROL JET SKI",
                "TWIN ROTARY ENGINES & RIVER MINES",
            ),
            Act::Act3_3SniperBoss => (
                "SNIPER KOOKY",
                "CYBORG LAUGHING HUNTER",
                "HIGH-CALIBER BROADCAST LASER RIFLE",
            ),
            Act::Act4_3ExcavatorBoss => (
                "DR. CANE TOAD",
                "TOAD-DOZER HEAVY EXCAVATOR",
                "HYDRAULIC CLAW & TOXIC SLIME MORTAR",
            ),
            _ => return,
        };

        let card_x: i16 = 20;
        let card_y: i16 = 145;
        let card_w: u16 = 280;
        let card_h: u16 = 62;

        gpu::draw_rect_flat(card_x, card_y, card_w, card_h, 12, 18, 24);
        gpu::draw_rect_flat(card_x + 2, card_y + 2, card_w - 4, card_h - 4, 4, 8, 12);
        gpu::draw_rect_flat(card_x, card_y, 4, card_h, 255, 60, 60); // Red warning accent strip

        self.font.draw_text(card_x + 12, card_y + 8, name, (255, 230, 80));
        self.font.draw_text(card_x + 12, card_y + 24, codename, (120, 255, 160));
        self.font.draw_text(card_x + 12, card_y + 40, specs, (200, 220, 240));

        if timer > 60 && (timer / 15) % 2 == 0 {
            self.font.draw_text(180, 222, "CROSS: SKIP", (160, 160, 160));
        }
    }
}
