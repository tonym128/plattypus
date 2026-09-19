//! Hardware GTE 3D perspective renderer, Soliton Radar, and MGS Tactical HUD.
//! Renders 3D environments, character meshes, dynamic vision cones, and searchlights.

use crate::entities::{AlertState, CollectibleType, EntityManager, SentryState};
use crate::level::{Act, CellType, Level, GRID_D, GRID_W, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};

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

        // One-shot GTE scene setup: 160x120 center, 200 focal length
        scene::set_screen_offset(160 << 16, 120 << 16);
        scene::set_projection_plane(200);

        Self {
            fb,
            font,
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
        // High-angle 3D perspective camera tracking Platty smoothly
        let target_x = player_x;
        let target_y = player_y - 300;
        let target_z = player_z - 260;

        // Smooth camera dampening
        self.cam_x += (target_x - self.cam_x) / 4;
        self.cam_y += (target_y - self.cam_y) / 4;
        self.cam_z += (target_z - self.cam_z) / 4;

        if self.screen_shake > 0 {
            let shake = if (self.screen_shake & 1) != 0 { self.screen_shake } else { -self.screen_shake };
            self.cam_x += shake as i32;
            self.cam_z += (shake / 2) as i32;
            self.screen_shake -= 1;
        }

        // Setup GTE rotation and translation for the 3D frame
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
    // 3D SCENE RENDERING
    // -------------------------------------------------------------------------

    pub fn draw_3d_scene(
        &self,
        level: &Level,
        platty: &Platypus,
        entities: &EntityManager,
        frame: u8,
    ) {
        // Dark military night sky backdrop
        gpu::draw_rect_flat(0, 0, 320, 240, 8, 12, 16);

        // 1. Render 3D Sector Ground & Environment
        self.draw_environment(level, frame);

        // 2. Render 3D Searchlights & Vision Cones on Ground (Blended)
        self.draw_vision_cones(entities);

        // 3. Render 3D Tactical Collectibles
        self.draw_collectibles(entities, frame);

        // 4. Render 3D Sentries & Drones
        self.draw_sentries(entities);
        self.draw_drones(entities, frame);

        // 5. Render 3D Plattypus Model
        self.draw_plattypus(platty);

        // 6. Render 3D Particles
        self.draw_particles(entities);
    }

    fn draw_environment(&self, level: &Level, frame: u8) {
        let (floor_r, floor_g, floor_b) = match level.act {
            Act::Act1Sanctuary => (25, 35, 40),   // Dark tarmac / concrete compound
            Act::Act2Bushland => (80, 50, 30),    // Outback red earth
            Act::Act3City => (45, 48, 52),        // Concrete loading docks
            Act::Act4Ocean => (130, 115, 80),     // Coastal beach sand
        };

        // Render visible grid cells in 3D
        let min_gx = ((self.cam_x - 300) / TILE_SZ).max(0) as usize;
        let max_gx = ((self.cam_x + 300) / TILE_SZ + 1).min(GRID_W as i32) as usize;
        let min_gz = ((self.cam_z + 100) / TILE_SZ).max(0) as usize;
        let max_gz = ((self.cam_z + 650) / TILE_SZ + 1).min(GRID_D as i32) as usize;

        for gz in min_gz..max_gz {
            let wz = (gz as i32) * TILE_SZ;
            for gx in min_gx..max_gx {
                let wx = (gx as i32) * TILE_SZ;
                let cell = level.get_cell(gx, gz);

                match cell {
                    CellType::Floor => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                    }
                    CellType::TallGrass => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // Australian bushgrass clumps for crawling stealth
                        self.draw_grass_clump(wx + 16, wz + 16);
                        self.draw_grass_clump(wx + 44, wz + 36);
                    }
                    CellType::Water => {
                        self.draw_water_tile(wx, wz, frame, false);
                    }
                    CellType::WaterCurrent => {
                        self.draw_water_tile(wx, wz, frame, true);
                    }
                    CellType::Crate => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // "MELBOURNE FRUIT CO" cargo crate (48x48x48 box)
                        self.draw_box_3d(wx + 8, wz + 8, 48, 48, 48, (115, 75, 40));
                    }
                    CellType::Container => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // Military freight container (60x60x70)
                        let c_col = match level.act {
                            Act::Act1Sanctuary => (45, 70, 55),
                            Act::Act2Bushland => (110, 60, 45),
                            Act::Act3City => (35, 75, 105),
                            Act::Act4Ocean => (60, 80, 95),
                        };
                        self.draw_box_3d(wx + 2, wz + 2, 60, 70, 60, c_col);
                    }
                    CellType::Wall => {
                        // High concrete security wall (64x64x80)
                        self.draw_box_3d(wx, wz, 64, 80, 64, (60, 65, 70));
                    }
                    CellType::AirDuct => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // Low ventilation duct / crawl pipe (28 units high)
                        self.draw_box_3d(wx + 8, wz + 8, 48, 26, 48, (70, 75, 80));
                    }
                    CellType::LaserTripwire => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // Red glowing security laser tripwire
                        self.draw_laser_tripwire(wx, wz, frame);
                    }
                    CellType::ExitBurrow => {
                        self.draw_floor_tile(wx, wz, floor_r, floor_g, floor_b);
                        // Glowing exit hatch
                        self.draw_exit_hatch(wx + 12, wz + 12, frame);
                    }
                }
            }
        }
    }

    fn draw_floor_tile(&self, wx: i32, wz: i32, r: u8, g: u8, b: u8) {
        let v0 = Vec3I16::new(wx as i16, 0, wz as i16);
        let v1 = Vec3I16::new((wx + TILE_SZ) as i16, 0, wz as i16);
        let v2 = Vec3I16::new(wx as i16, 0, (wz + TILE_SZ) as i16);
        let v3 = Vec3I16::new((wx + TILE_SZ) as i16, 0, (wz + TILE_SZ) as i16);
        Self::draw_quad_3d(v0, v1, v2, v3, r, g, b);
    }

    fn draw_water_tile(&self, wx: i32, wz: i32, frame: u8, is_current: bool) {
        let bob = if (frame / 8) % 2 == 0 { 2 } else { 0 };
        let y = 14 + bob;
        let v0 = Vec3I16::new(wx as i16, y, wz as i16);
        let v1 = Vec3I16::new((wx + TILE_SZ) as i16, y, wz as i16);
        let v2 = Vec3I16::new(wx as i16, y, (wz + TILE_SZ) as i16);
        let v3 = Vec3I16::new((wx + TILE_SZ) as i16, y, (wz + TILE_SZ) as i16);

        let (r, g, b) = if is_current {
            (35, 115, 195)
        } else {
            (25, 80, 150)
        };
        Self::draw_quad_3d(v0, v1, v2, v3, r, g, b);
    }

    fn draw_grass_clump(&self, wx: i32, wz: i32) {
        let p0 = scene::project_vertex(Vec3I16::new((wx - 10) as i16, 0, wz as i16));
        let p1 = scene::project_vertex(Vec3I16::new((wx + 10) as i16, 0, wz as i16));
        let p2 = scene::project_vertex(Vec3I16::new(wx as i16, -20, wz as i16));
        if p0.sz > 20 && p1.sz > 20 && p2.sz > 20 {
            gpu::draw_tri_flat([(p0.sx, p0.sy), (p1.sx, p1.sy), (p2.sx, p2.sy)], 45, 125, 55);
        }
    }

    fn draw_laser_tripwire(&self, wx: i32, wz: i32, frame: u8) {
        // Laser emitter posts on sides
        self.draw_box_3d(wx + 2, wz + 28, 8, 24, 8, (90, 95, 105));
        self.draw_box_3d(wx + 54, wz + 28, 8, 24, 8, (90, 95, 105));

        // Red glowing beam
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
        Self::draw_quad_3d(v0, v1, v2, v3, 110, 80, 50);

        // Golden goal beacon light
        let p = scene::project_vertex(Vec3I16::new((wx + 20) as i16, -15, (wz + 20) as i16));
        if p.sz > 20 {
            let spark = if (frame / 8) % 2 == 0 { 255 } else { 170 };
            gpu::draw_rect_flat(p.sx - 4, p.sy - 4, 8, 8, spark, spark, 80);
        }
    }

    /// Draw shaded 3D box with directional lighting and backface culling.
    pub fn draw_box_3d(&self, wx: i32, wz: i32, w: i32, h: i32, d: i32, col: (u8, u8, u8)) {
        let x0 = wx as i16;
        let x1 = (wx + w) as i16;
        let y0 = -h as i16;
        let y1 = 0i16;
        let z0 = wz as i16;
        let z1 = (wz + d) as i16;

        // 1. TOP FACE (lit directly from above: 100% brightness)
        let t0 = Vec3I16::new(x0, y0, z0);
        let t1 = Vec3I16::new(x1, y0, z0);
        let t2 = Vec3I16::new(x0, y0, z1);
        let t3 = Vec3I16::new(x1, y0, z1);
        Self::draw_quad_3d(t0, t1, t2, t3, col.0, col.1, col.2);

        // 2. FRONT FACE (facing camera south: 85% brightness)
        let f_r = (col.0 as u16 * 85 / 100) as u8;
        let f_g = (col.1 as u16 * 85 / 100) as u8;
        let f_b = (col.2 as u16 * 85 / 100) as u8;
        let f0 = Vec3I16::new(x0, y0, z1);
        let f1 = Vec3I16::new(x1, y0, z1);
        let f2 = Vec3I16::new(x0, y1, z1);
        let f3 = Vec3I16::new(x1, y1, z1);
        Self::draw_quad_3d(f0, f1, f2, f3, f_r, f_g, f_b);

        // 3. LEFT FACE (side face: 70% brightness)
        let l_r = (col.0 as u16 * 70 / 100) as u8;
        let l_g = (col.1 as u16 * 70 / 100) as u8;
        let l_b = (col.2 as u16 * 70 / 100) as u8;
        let l0 = Vec3I16::new(x0, y0, z0);
        let l1 = Vec3I16::new(x0, y0, z1);
        let l2 = Vec3I16::new(x0, y1, z0);
        let l3 = Vec3I16::new(x0, y1, z1);
        Self::draw_quad_3d(l0, l1, l2, l3, l_r, l_g, l_b);

        // 4. RIGHT FACE (shadowed side: 60% brightness)
        let r_r = (col.0 as u16 * 60 / 100) as u8;
        let r_g = (col.1 as u16 * 60 / 100) as u8;
        let r_b = (col.2 as u16 * 60 / 100) as u8;
        let r0 = Vec3I16::new(x1, y0, z1);
        let r1 = Vec3I16::new(x1, y0, z0);
        let r2 = Vec3I16::new(x1, y1, z1);
        let r3 = Vec3I16::new(x1, y1, z0);
        Self::draw_quad_3d(r0, r1, r2, r3, r_r, r_g, r_b);
    }

    /// Project and render a 3D quad using two triangles with screen backface culling.
    #[inline]
    pub fn draw_quad_3d(v0: Vec3I16, v1: Vec3I16, v2: Vec3I16, v3: Vec3I16, r: u8, g: u8, b: u8) {
        let p0 = scene::project_vertex(v0);
        let p1 = scene::project_vertex(v1);
        let p2 = scene::project_vertex(v2);
        let p3 = scene::project_vertex(v3);

        if p0.sz < 20 || p1.sz < 20 || p2.sz < 20 || p3.sz < 20 {
            return;
        }

        // Screen-space backface culling
        let ax = p1.sx as i32 - p0.sx as i32;
        let ay = p1.sy as i32 - p0.sy as i32;
        let bx = p2.sx as i32 - p0.sx as i32;
        let by = p2.sy as i32 - p0.sy as i32;
        if ax * by - ay * bx <= 0 {
            return;
        }

        gpu::draw_tri_flat([(p0.sx, p0.sy), (p1.sx, p1.sy), (p2.sx, p2.sy)], r, g, b);
        gpu::draw_tri_flat([(p1.sx, p1.sy), (p3.sx, p3.sy), (p2.sx, p2.sy)], r, g, b);
    }

    // -------------------------------------------------------------------------
    // VISION CONES & SEARCHLIGHTS
    // -------------------------------------------------------------------------

    fn draw_vision_cones(&self, entities: &EntityManager) {
        // Searchlight illumination pools on the floor
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

        // Sentry Vision Cones
        for s in entities.sentries.iter() {
            if !s.active || s.stun_timer > 0 {
                continue;
            }

            let sx = s.x as i16;
            let sz = s.z as i16;

            // Cone extends 180 units forward with 30-degree half angle (22 in 256 angle units)
            let left_ang = s.angle.wrapping_sub(22);
            let right_ang = s.angle.wrapping_add(22);

            let l_x = sx + ((cos_1_3_12(left_ang) as i32 * 180) >> 12) as i16;
            let l_z = sz + ((sin_1_3_12(left_ang) as i32 * 180) >> 12) as i16;

            let r_x = sx + ((cos_1_3_12(right_ang) as i32 * 180) >> 12) as i16;
            let r_z = sz + ((sin_1_3_12(right_ang) as i32 * 180) >> 12) as i16;

            let p_origin = scene::project_vertex(Vec3I16::new(sx, 1, sz));
            let p_left = scene::project_vertex(Vec3I16::new(l_x, 1, l_z));
            let p_right = scene::project_vertex(Vec3I16::new(r_x, 1, r_z));

            if p_origin.sz > 20 && p_left.sz > 20 && p_right.sz > 20 {
                let (cr, cg, cb) = if s.see_player {
                    (200, 30, 30) // Red alert cone!
                } else if matches!(s.state, SentryState::Investigating) {
                    (180, 150, 30) // Amber caution cone!
                } else {
                    (40, 130, 60) // Green stealth cone!
                };

                gpu::draw_tri_flat_blended(
                    [(p_origin.sx, p_origin.sy), (p_left.sx, p_left.sy), (p_right.sx, p_right.sy)],
                    cr, cg, cb, BlendMode::Add,
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // 3D CHARACTERS & DRONES
    // -------------------------------------------------------------------------

    fn draw_plattypus(&self, platty: &Platypus) {
        if platty.invuln_timer > 0 && (platty.invuln_timer / 4) % 2 == 1 {
            return;
        }

        let px = platty.x;
        let py = platty.y;
        let pz = platty.z;

        let rot = Mat3I16::rotate_y(platty.angle);

        // Model dimensions in local space
        let is_crawl = platty.state == PlayerState::BellyCrawl;
        let body_h = if is_crawl { 8 } else { 18 };

        // 1. Platty Body (Brown fur)
        self.draw_model_box(px, py, pz, -12, -body_h, -14, 24, body_h, 28, &rot, (100, 60, 32));

        // 2. Duck Bill protruding forward (+Z in local facing)
        self.draw_model_box(px, py, pz, -7, -10, 14, 14, 6, 16, &rot, (45, 42, 42));

        // 3. Beaver Paddle Tail trailing behind (-Z in local facing)
        let tail_wobble = ((platty.anim_frame / 4) % 2) as i32 * 2;
        self.draw_model_box(px, py, pz, -8 + tail_wobble, -6, -30, 16, 5, 18, &rot, (75, 42, 22));

        // 4. Leather Satchel with Important Letter!
        self.draw_model_box(px, py, pz, 4, -body_h - 4, -4, 9, 8, 10, &rot, (175, 130, 65));
        self.draw_model_box(px, py, pz, 6, -body_h - 6, -2, 5, 3, 6, &rot, (255, 250, 240));

        // 5. Solid Snake Green Bandana around forehead!
        self.draw_model_box(px, py, pz, -13, -body_h - 1, 2, 26, 4, 12, &rot, (35, 135, 60));
        // Fluttering knot
        let knot_flutter = if (platty.anim_frame / 6) % 2 == 0 { -4 } else { 2 };
        self.draw_model_box(px, py, pz, -16 + knot_flutter, -body_h, -10, 6, 8, 8, &rot, (30, 120, 55));
    }

    fn draw_sentries(&self, entities: &EntityManager) {
        for s in entities.sentries.iter() {
            if !s.active {
                continue;
            }

            let sx = s.x;
            let sy = s.y;
            let sz = s.z;
            let rot = Mat3I16::rotate_y(s.angle);

            if s.stun_timer > 0 {
                // Knocked out sentry lying on floor
                self.draw_model_box(sx, sy, sz, -12, -6, -18, 24, 6, 36, &rot, (45, 60, 75));
                // Circling knockout stars overhead
                let star_p = scene::project_vertex(Vec3I16::new(sx as i16, -18, sz as i16));
                if star_p.sz > 20 {
                    let star_off = ((entities.frame * 4) % 20) as i16 - 10;
                    gpu::draw_rect_flat(star_p.sx + star_off, star_p.sy - 8, 4, 4, 255, 240, 60);
                }
            } else {
                // Standing sentry (legs, torso, ranger hat/helmet, rifle)
                self.draw_model_box(sx, sy, sz, -8, -16, -8, 16, 16, 16, &rot, (40, 48, 55)); // Legs
                self.draw_model_box(sx, sy, sz, -10, -32, -8, 20, 16, 16, &rot, (55, 75, 65)); // Torso
                self.draw_model_box(sx, sy, sz, -8, -44, -8, 16, 12, 16, &rot, (190, 160, 130)); // Head
                self.draw_model_box(sx, sy, sz, -12, -48, -12, 24, 6, 24, &rot, (75, 65, 45)); // Ranger Hat
                self.draw_model_box(sx, sy, sz, 8, -26, 4, 6, 6, 22, &rot, (30, 32, 35)); // Rifle

                // Big Red "!" Exclamation mark overhead when player detected!
                if s.see_player {
                    let p = scene::project_vertex(Vec3I16::new(sx as i16, -65, sz as i16));
                    if p.sz > 20 {
                        // Red exclamation mark badge
                        gpu::draw_rect_flat(p.sx - 3, p.sy - 16, 6, 12, 240, 20, 30);
                        gpu::draw_rect_flat(p.sx - 3, p.sy - 2, 6, 4, 240, 20, 30);
                        gpu::draw_rect_flat(p.sx - 1, p.sy - 14, 2, 8, 255, 220, 220); // highlight
                    }
                }
            }
        }
    }

    fn draw_drones(&self, entities: &EntityManager, frame: u8) {
        for d in entities.drones.iter() {
            if !d.active {
                continue;
            }
            let rot = Mat3I16::rotate_y(d.angle);
            // Spherical drone body
            self.draw_model_box(d.x, d.y, d.z, -10, -10, -10, 20, 20, 20, &rot, (60, 65, 75));
            // Camera lens
            self.draw_model_box(d.x, d.y, d.z, -4, -4, 10, 8, 8, 4, &rot, (220, 40, 40));

            // Spinning twin rotor blades
            let p = scene::project_vertex(Vec3I16::new(d.x as i16, (d.y - 12) as i16, d.z as i16));
            if p.sz > 20 {
                let r_off = if (frame / 2) % 2 == 0 { 14 } else { -14 };
                gpu::draw_line_mono(p.sx - r_off, p.sy, p.sx + r_off, p.sy, 200, 210, 220);
            }
        }
    }

    fn draw_collectibles(&self, entities: &EntityManager, frame: u8) {
        let bob = if (frame / 6) % 2 == 0 { 2 } else { 0 };
        for c in entities.collectibles.iter() {
            if !c.active || !c.revealed {
                continue;
            }
            let p = scene::project_vertex(Vec3I16::new(c.x as i16, (c.y - bob) as i16, c.z as i16));
            if p.sz > 20 {
                match c.kind {
                    CollectibleType::YabbyRation => {
                        // Blue/gold delicious yabby ration
                        gpu::draw_rect_flat(p.sx - 5, p.sy - 5, 10, 10, 45, 140, 240);
                        gpu::draw_rect_flat(p.sx - 3, p.sy - 3, 6, 6, 255, 230, 80);
                    }
                    CollectibleType::ChaffBattery => {
                        // Green battery pack
                        gpu::draw_rect_flat(p.sx - 4, p.sy - 6, 8, 12, 60, 220, 90);
                        gpu::draw_rect_flat(p.sx - 2, p.sy - 8, 4, 3, 200, 200, 200);
                    }
                    CollectibleType::LetterPage => {
                        // Parents' intelligence letter envelope
                        gpu::draw_rect_flat(p.sx - 6, p.sy - 4, 12, 9, 250, 245, 230);
                        gpu::draw_rect_flat(p.sx - 2, p.sy - 2, 4, 4, 210, 45, 45); // red wax seal
                    }
                    CollectibleType::BuriedYabby => {
                        // Glowing rare golden yabby
                        let spark = if (frame / 4) % 2 == 0 { 255 } else { 180 };
                        gpu::draw_rect_flat(p.sx - 6, p.sy - 6, 12, 12, spark, spark, 60);
                    }
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

    /// Helper to draw a local 3D box transformed by a rotation and placed at world position.
    fn draw_model_box(
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
        let corners = [
            (lx, ly, lz),
            (lx + w, ly, lz),
            (lx, ly, lz + d),
            (lx + w, ly, lz + d),
            (lx, ly + h, lz),
            (lx + w, ly + h, lz),
            (lx, ly + h, lz + d),
            (lx + w, ly + h, lz + d),
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

        // Top face
        Self::draw_quad_3d(world_pts[0], world_pts[1], world_pts[2], world_pts[3], col.0, col.1, col.2);
        // Front face
        let f_col = (col.0 * 85 / 100, col.1 * 85 / 100, col.2 * 85 / 100);
        Self::draw_quad_3d(world_pts[2], world_pts[3], world_pts[6], world_pts[7], f_col.0, f_col.1, f_col.2);
        // Left face
        let l_col = (col.0 * 70 / 100, col.1 * 70 / 100, col.2 * 70 / 100);
        Self::draw_quad_3d(world_pts[0], world_pts[2], world_pts[4], world_pts[6], l_col.0, l_col.1, l_col.2);
        // Right face
        let r_col = (col.0 * 60 / 100, col.1 * 60 / 100, col.2 * 60 / 100);
        Self::draw_quad_3d(world_pts[3], world_pts[1], world_pts[7], world_pts[5], r_col.0, r_col.1, r_col.2);
    }

    // -------------------------------------------------------------------------
    // TACTICAL MGS HUD & SOLITON RADAR
    // -------------------------------------------------------------------------

    pub fn draw_hud(&self, platty: &Platypus, entities: &EntityManager, act: Act) {
        // TOP-LEFT: LIFE GAUGE & O2 METER
        gpu::draw_rect_flat(8, 8, 120, 36, 12, 18, 24);
        gpu::draw_rect_flat(10, 10, 116, 32, 4, 8, 12);

        self.font.draw_text(14, 12, "LIFE", (230, 50, 50));
        // Segmented Life Bar
        for i in 0..platty.max_health {
            let lx = 48 + i as i16 * 18;
            if i < platty.health {
                gpu::draw_rect_flat(lx, 13, 14, 8, 235, 45, 45);
                gpu::draw_rect_flat(lx + 1, 14, 12, 2, 255, 180, 180);
            } else {
                gpu::draw_rect_flat(lx, 13, 14, 8, 60, 30, 35);
            }
        }

        // Air Meter when in water / submerged
        if platty.y > 0 || platty.state == PlayerState::Swimming || platty.state == PlayerState::Submerged {
            self.font.draw_text(14, 26, "O2", (70, 210, 255));
            gpu::draw_rect_flat(48, 27, 60, 7, 20, 35, 55);
            let o2_w = (platty.air as u32 * 58 / 100) as u16;
            let o2_col = if platty.air < 30 { (255, 60, 50) } else { (80, 220, 255) };
            gpu::draw_rect_flat(49, 28, o2_w, 5, o2_col.0, o2_col.1, o2_col.2);
        } else {
            // Act title badge
            self.font.draw_text(14, 26, act.title(), (240, 210, 100));
        }

        // Score & Yabbies display
        let mut score_str = [b'0'; 6];
        let mut sc = platty.score;
        for i in (0..6).rev() {
            score_str[i] = (sc % 10) as u8 + b'0';
            sc /= 10;
        }
        if let Ok(s) = core::str::from_utf8(&score_str) {
            self.font.draw_text(138, 12, s, (240, 240, 240));
        }

        let mut yabbies_buf = [b'Y', b'A', b'B', b':', b'x', b'0', b'0', 0];
        yabbies_buf[5] = ((platty.yabbies_collected / 10) % 10) as u8 + b'0';
        yabbies_buf[6] = (platty.yabbies_collected % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&yabbies_buf[..7]) {
            self.font.draw_text(138, 26, s, (120, 210, 255));
        }

        // Call button prompt bottom-left
        self.font.draw_text(14, 222, "[SELECT] CALL BURROW HQ", (120, 255, 160));

        // TOP-RIGHT: SOLITON RADAR (MGS CLASSIC)
        self.draw_soliton_radar(platty, entities);
    }

    fn draw_soliton_radar(&self, platty: &Platypus, entities: &EntityManager) {
        let rx: i16 = 244;
        let ry: i16 = 8;
        let rw: i16 = 68;
        let rh: i16 = 68;

        // Radar frame
        gpu::draw_rect_flat(rx, ry, rw as u16, rh as u16, 10, 35, 25);
        gpu::draw_rect_flat(rx + 2, ry + 2, (rw - 4) as u16, (rh - 4) as u16, 4, 18, 12);

        // Check if Jammed / In Red Alert
        if let AlertState::Alert(timer) = entities.alert_state {
            // Soliton Radar is JAMMED! Flashing red static noise lines
            let red_flash = if (entities.frame / 8) % 2 == 0 { 240 } else { 160 };
            gpu::draw_rect_flat(rx + 4, ry + 4, (rw - 8) as u16, (rh - 8) as u16, red_flash / 4, 8, 8);

            // Static noise bars
            for line in 0..6 {
                let ly = ry + 8 + line * 9 + ((entities.frame as i16 * 7) % 7);
                gpu::draw_rect_flat(rx + 6, ly, (rw - 12) as u16, 2, red_flash, 30, 30);
            }

            self.font.draw_text(rx + 12, ry + 16, "ALERT", (255, 50, 50));
            let mut time_str = [b'0', b'0', b'.', b'0', 0];
            let secs = (timer / 60) as u8;
            let frac = ((timer % 60) * 10 / 60) as u8;
            time_str[0] = (secs / 10) + b'0';
            time_str[1] = (secs % 10) + b'0';
            time_str[3] = frac + b'0';
            if let Ok(s) = core::str::from_utf8(&time_str[..4]) {
                self.font.draw_text(rx + 16, ry + 36, s, (255, 60, 60));
            }
            return;
        }

        // Radar grid lines
        gpu::draw_line_mono(rx + rw / 2, ry + 4, rx + rw / 2, ry + rh - 4, 15, 60, 35);
        gpu::draw_line_mono(rx + 4, ry + rh / 2, rx + rw - 4, ry + rh / 2, 15, 60, 35);

        // Center dot: Platty
        let center_x = rx + rw / 2;
        let center_y = ry + rh / 2;
        gpu::draw_rect_flat(center_x - 1, center_y - 1, 3, 3, 255, 255, 255);

        // Direction chevron indicator
        let dir_x = (cos_1_3_12(platty.angle) as i32 * 5) >> 12;
        let dir_z = (sin_1_3_12(platty.angle) as i32 * 5) >> 12;
        gpu::draw_rect_flat((center_x as i32 + dir_x) as i16, (center_y as i32 + dir_z) as i16, 2, 2, 80, 255, 140);

        // Radar Scale: 1 radar pixel = 16 world units
        let scale = 16;

        // Draw Sentries on Radar
        for s in entities.sentries.iter() {
            if !s.active {
                continue;
            }
            let sx = center_x as i32 + (s.x - platty.x) / scale;
            let sy = center_y as i32 + (s.z - platty.z) / scale;

            if sx >= (rx + 4) as i32 && sx < (rx + rw - 4) as i32 && sy >= (ry + 4) as i32 && sy < (ry + rh - 4) as i32 {
                if s.stun_timer > 0 {
                    gpu::draw_rect_flat(sx as i16 - 1, sy as i16 - 1, 3, 3, 110, 110, 110);
                } else {
                    // Red sentry dot
                    gpu::draw_rect_flat(sx as i16 - 1, sy as i16 - 1, 3, 3, 255, 40, 40);

                    // Mini vision cone on radar
                    let cone_x = (cos_1_3_12(s.angle) as i32 * 8) >> 12;
                    let cone_z = (sin_1_3_12(s.angle) as i32 * 8) >> 12;
                    gpu::draw_line_mono(sx as i16, sy as i16, (sx + cone_x) as i16, (sy + cone_z) as i16, 220, 200, 50);
                }
            }
        }

        // Draw Drones on Radar
        for d in entities.drones.iter() {
            if !d.active {
                continue;
            }
            let dx = center_x as i32 + (d.x - platty.x) / scale;
            let dy = center_y as i32 + (d.z - platty.z) / scale;
            if dx >= (rx + 4) as i32 && dx < (rx + rw - 4) as i32 && dy >= (ry + 4) as i32 && dy < (ry + rh - 4) as i32 {
                gpu::draw_rect_flat(dx as i16 - 1, dy as i16 - 1, 3, 3, 240, 140, 40);
            }
        }

        // Caution indicator on top of radar
        if let AlertState::Caution(_timer) = entities.alert_state {
            self.font.draw_text(rx + 8, ry - 7, "CAUTION", (255, 200, 60));
        }
    }

    // -------------------------------------------------------------------------
    // TITLE SCREEN, MISSION CLEAR & CUTSCENES
    // -------------------------------------------------------------------------

    pub fn draw_title_screen(&self, frame: u8) {
        // Dark military blueprint scanline aesthetic
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 14, 20);

        for y in (0..240).step_by(6) {
            gpu::draw_rect_flat(0, y, 320, 1, 10, 22, 32);
        }

        // Title tactical border
        gpu::draw_rect_flat(25, 20, 270, 70, 15, 45, 60);
        gpu::draw_rect_flat(27, 22, 266, 66, 8, 18, 26);

        let glow = if (frame / 12) % 2 == 0 { (255, 230, 80) } else { (235, 195, 60) };
        self.font.draw_text(90, 32, "P L A T T Y P U S", glow);
        self.font.draw_text(42, 52, "TACTICAL ESPIONAGE INFILTRATION", (120, 230, 255));
        self.font.draw_text(58, 68, "JOURNEY HOME TO THE NATIVE WILD", (160, 190, 220));

        // Center tactical silhouette illustration of Platty with glowing bandana & eyes
        let cx = 145;
        let cy = 115;
        gpu::draw_rect_flat(cx, cy, 30, 16, 95, 55, 30);
        gpu::draw_rect_flat(cx + 25, cy + 4, 14, 8, 45, 42, 42); // Bill
        gpu::draw_rect_flat(cx - 16, cy + 3, 18, 10, 75, 42, 22); // Tail
        gpu::draw_rect_flat(cx + 8, cy - 5, 12, 10, 175, 130, 65); // Satchel with Letter
        gpu::draw_rect_flat(cx + 10, cy - 8, 8, 5, 255, 250, 235);
        gpu::draw_rect_flat(cx + 18, cy + 2, 4, 4, 100, 255, 220); // Night vision glow

        // Green headband
        gpu::draw_rect_flat(cx + 12, cy - 2, 16, 4, 40, 180, 80);
        let flutter = if (frame / 8) % 2 == 0 { 2 } else { -2 };
        gpu::draw_rect_flat(cx + 6 + flutter, cy - 1, 8, 10, 35, 160, 70);

        // Blinking start prompt
        if (frame / 20) % 2 == 0 {
            self.font.draw_text(85, 165, "PRESS START OR CROSS", (255, 255, 255));
        }

        self.font.draw_text(60, 205, "CODEC RADIO SYSTEM ENGAGED: 140.85", (90, 210, 140));
    }

    pub fn draw_stage_clear(&self, act: Act, score: u32, yabbies: u16) {
        gpu::draw_rect_flat(25, 30, 270, 180, 12, 40, 30);
        gpu::draw_rect_flat(28, 33, 264, 174, 6, 20, 16);

        self.font.draw_text(55, 50, "INFILTRATION COMPLETE!", (100, 255, 140));
        self.font.draw_text(40, 75, act.title(), (255, 225, 80));

        let mut score_str = [b'S', b'C', b'O', b'R', b'E', b':', b' ', b'0', b'0', b'0', b'0', b'0', b'0', 0];
        let mut sc = score;
        for i in (7..13).rev() {
            score_str[i] = (sc % 10) as u8 + b'0';
            sc /= 10;
        }
        if let Ok(s) = core::str::from_utf8(&score_str[..13]) {
            self.font.draw_text(55, 105, s, (240, 240, 240));
        }

        let mut yab_str = [b'Y', b'A', b'B', b'B', b'I', b'E', b'S', b':', b' ', b'0', b'0', 0];
        yab_str[9] = ((yabbies / 10) % 10) as u8 + b'0';
        yab_str[10] = (yabbies % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&yab_str[..11]) {
            self.font.draw_text(55, 125, s, (120, 210, 255));
        }

        self.font.draw_text(55, 145, "STEALTH RANK: FOX HOUND", (255, 215, 90));
        self.font.draw_text(55, 175, "PRESS CROSS TO CONTACT BURROW", (255, 255, 255));
    }

    pub fn draw_ending(&self, frame: u8) {
        // Sunset Coastal Burrow
        gpu::draw_rect_flat(0, 0, 320, 120, 235, 110, 70);
        gpu::draw_rect_flat(0, 120, 320, 120, 200, 160, 110);
        gpu::draw_rect_flat(0, 180, 320, 60, 40, 110, 160); // Estuary

        // Family Burrow
        gpu::draw_rect_flat(120, 130, 80, 40, 110, 75, 45);
        gpu::draw_rect_flat(135, 140, 50, 30, 30, 20, 15);

        // Mom & Dad Platypus
        gpu::draw_rect_flat(90, 145, 24, 12, 95, 55, 30); // Mom
        gpu::draw_rect_flat(82, 148, 9, 6, 45, 42, 42); // Bill
        gpu::draw_rect_flat(205, 145, 24, 12, 95, 55, 30); // Dad
        gpu::draw_rect_flat(228, 148, 9, 6, 45, 42, 42); // Bill

        // Platty with bandana
        gpu::draw_rect_flat(148, 145, 20, 10, 95, 55, 30);
        gpu::draw_rect_flat(152, 142, 12, 3, 35, 140, 60); // Bandana

        // Baby Sister Pip (hatched puggle!)
        let egg_bob = if (frame / 10) % 2 == 0 { 1 } else { 0 };
        gpu::draw_rect_flat(152, 134 - egg_bob, 12, 10, 255, 250, 220); // Shell
        gpu::draw_rect_flat(155, 136 - egg_bob, 6, 5, 135, 95, 60); // Pip's cute head!

        // Mission complete banner
        gpu::draw_rect_flat(20, 20, 280, 70, 15, 35, 45);
        gpu::draw_rect_flat(22, 22, 276, 66, 10, 20, 25);
        self.font.draw_text(35, 30, "MOM & DAD: PLATTY! MISSION ACCOMPLISHED!", (255, 230, 90));
        self.font.draw_text(35, 50, "PIP HATCHED SAFELY IN THE BURROW!", (100, 255, 180));
        self.font.draw_text(35, 70, "CONGRATULATIONS, BIG BROTHER!", (255, 255, 255));
    }
}
