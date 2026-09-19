//! Hardware-accelerated PSX GPU renderer for Plattypus.

use crate::entities::{CollectibleType, EnemyType, EntityManager};
use crate::fixed::Fixed;
use crate::level::{Act, Level, TileType, LEVEL_H, LEVEL_W, TILE_SIZE};
use crate::platypus::{PlayerState, Platypus};

use psx_font::{fonts::BASIC, FontAtlas};
use psx_gpu::{
    self as gpu,
    framebuf::FrameBuffer,
    material::BlendMode,
    Resolution, VideoMode,
};
use psx_vram::{Clut, TexDepth, Tpage};

pub const SCREEN_W: i16 = 320;
pub const SCREEN_H: i16 = 240;

const FONT_TPAGE: Tpage = Tpage::new(320, 0, TexDepth::Bit4);
const FONT_CLUT: Clut = Clut::new(320, 256);

pub struct Renderer {
    pub fb: FrameBuffer,
    pub font: FontAtlas,
    pub camera_x: i32,
}

impl Renderer {
    pub fn new() -> Self {
        gpu::init(VideoMode::Ntsc, Resolution::R320X240);
        let fb = FrameBuffer::new(320, 240);
        gpu::set_draw_area(0, 0, 319, 239);
        gpu::set_draw_offset(0, 0);

        let font = FontAtlas::upload(&BASIC, FONT_TPAGE, FONT_CLUT);

        Self {
            fb,
            font,
            camera_x: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        psx_rt::interrupts::wait_vblank();
        self.fb.swap();
    }

    pub fn update_camera(&mut self, player_x: Fixed) {
        let px = player_x.to_int();
        // Camera centers on player
        let target_x = px - (SCREEN_W as i32 / 2);
        let max_cam = (LEVEL_W as i32 * TILE_SIZE) - SCREEN_W as i32;

        if target_x < 0 {
            self.camera_x = 0;
        } else if target_x > max_cam {
            self.camera_x = max_cam;
        } else {
            self.camera_x = target_x;
        }
    }

    /// Render sky and parallax background according to current Act.
    pub fn draw_background(&self, act: Act, frame: u8) {
        match act {
            Act::Act1Sanctuary => {
                // Night sky gradient (Midnight Navy -> Dark Indigo)
                gpu::draw_rect_flat(0, 0, 320, 120, 10, 15, 35);
                gpu::draw_rect_flat(0, 120, 320, 120, 15, 25, 50);

                // Moon
                gpu::draw_rect_flat(260, 25, 20, 20, 240, 245, 220);
                gpu::draw_rect_flat(258, 27, 24, 16, 240, 245, 220);

                // Twinkling stars
                let stars: [(i16, i16); 6] = [(40, 30), (85, 15), (140, 45), (195, 22), (230, 60), (290, 40)];
                for (sx, sy) in stars.iter() {
                    let brightness = if (frame + (*sx as u8)) % 40 < 20 { 255 } else { 160 };
                    gpu::draw_rect_flat(*sx, *sy, 2, 2, brightness, brightness, brightness);
                }

                // Parallax gum trees silhouette
                let tree_offset = (self.camera_x / 4) as i16;
                for i in 0..6 {
                    let tx = (i * 70 - tree_offset) % 360 - 20;
                    gpu::draw_rect_flat(tx, 90, 8, 50, 12, 20, 25);
                    gpu::draw_rect_flat(tx - 12, 65, 32, 30, 8, 16, 20);
                }
            }
            Act::Act2Bushland => {
                // Dawn / Early Morning twilight gradient (Amber -> Soft Violet)
                gpu::draw_rect_flat(0, 0, 320, 70, 70, 50, 80);
                gpu::draw_rect_flat(0, 70, 320, 70, 140, 90, 70);
                gpu::draw_rect_flat(0, 140, 320, 100, 180, 140, 80);

                // Distant rolling hills silhouette
                let hill_offset = (self.camera_x / 3) as i16;
                for i in 0..5 {
                    let hx = (i * 90 - hill_offset) % 400 - 40;
                    gpu::draw_tri_flat([(hx, 160), (hx + 45, 110), (hx + 90, 160)], 45, 60, 35);
                }
            }
            Act::Act3City => {
                // Urban Melbourne industrial night (Dark Charcoal -> Cyan tint)
                gpu::draw_rect_flat(0, 0, 320, 130, 20, 25, 30);
                gpu::draw_rect_flat(0, 130, 320, 110, 30, 40, 45);

                // City skyscraper silhouettes
                let bldg_offset = (self.camera_x / 4) as i16;
                for i in 0..7 {
                    let bx = (i * 55 - bldg_offset) % 380 - 30;
                    let bh = 70 + (i * 13 % 40);
                    gpu::draw_rect_flat(bx, 150 - bh, 35, bh as u16, 15, 18, 22);
                    // Yellow/cyan lighted windows
                    if i % 2 == 0 {
                        gpu::draw_rect_flat(bx + 8, 150 - bh + 15, 4, 6, 220, 200, 100);
                        gpu::draw_rect_flat(bx + 20, 150 - bh + 25, 4, 6, 100, 220, 240);
                    }
                }
            }
            Act::Act4Ocean => {
                // Coastal Sunrise (Golden Peach -> Ocean Turquoise)
                gpu::draw_rect_flat(0, 0, 320, 60, 230, 140, 100);
                gpu::draw_rect_flat(0, 60, 320, 60, 240, 190, 130);
                gpu::draw_rect_flat(0, 120, 320, 120, 40, 110, 140);

                // Rising morning sun
                gpu::draw_rect_flat(140, 45, 40, 30, 255, 230, 150);

                // Distant ocean waves
                let wave_offset = ((self.camera_x / 2) as i16 + (frame / 2) as i16) % 32;
                for i in 0..11 {
                    let wx = i * 32 - wave_offset;
                    gpu::draw_rect_flat(wx, 140, 16, 3, 100, 180, 200);
                }
            }
        }
    }

    /// Render visible level tiles within camera frustum.
    pub fn draw_level(&self, level: &Level, frame: u8) {
        let start_col = (self.camera_x / TILE_SIZE).max(0);
        let end_col = ((self.camera_x + SCREEN_W as i32) / TILE_SIZE + 1).min(LEVEL_W as i32);

        for tx in start_col..end_col {
            let screen_x = (tx * TILE_SIZE - self.camera_x) as i16;

            for ty in 0..LEVEL_H as i32 {
                let screen_y = (ty * TILE_SIZE) as i16;
                let tile = level.get_tile(tx, ty);

                match tile {
                    TileType::Empty => {}
                    TileType::Solid => {
                        self.draw_solid_tile(level.act, screen_x, screen_y, tx, ty);
                    }
                    TileType::SlopeDown => {
                        // 45 degree downhill slope
                        let c = self.act_ground_color(level.act);
                        gpu::draw_tri_flat(
                            [
                                (screen_x, screen_y),
                                (screen_x + 16, screen_y + 16),
                                (screen_x, screen_y + 16),
                            ],
                            c.0,
                            c.1,
                            c.2,
                        );
                    }
                    TileType::Platform => {
                        // Thin wooden plank or metal girder
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 4, 150, 105, 60);
                        gpu::draw_rect_flat(screen_x + 4, screen_y + 4, 2, 8, 100, 70, 40);
                        gpu::draw_rect_flat(screen_x + 10, screen_y + 4, 2, 8, 100, 70, 40);
                    }
                    TileType::Water => {
                        // Translucent water body with moving surface ripple
                        let ripple = if (frame / 8 + tx as u8) % 2 == 0 { 2 } else { 0 };
                        gpu::draw_rect_flat(screen_x, screen_y + ripple, 16, 16, 30, 90, 160);
                        // Surface wave highlight
                        gpu::draw_rect_flat(screen_x, screen_y + ripple, 16, 2, 100, 180, 240);
                    }
                    TileType::Hazard => {
                        // Spikes or electrical hazard
                        gpu::draw_tri_flat(
                            [(screen_x, screen_y + 16), (screen_x + 8, screen_y + 4), (screen_x + 16, screen_y + 16)],
                            220,
                            60,
                            40,
                        );
                    }
                    TileType::Exit => {
                        // Glowing transition archway / burrow entrance
                        gpu::draw_rect_flat(screen_x, screen_y - 8, 20, 24, 60, 40, 25);
                        gpu::draw_rect_flat(screen_x + 4, screen_y - 4, 12, 20, 20, 15, 10);
                        // Golden goal sparkle
                        let sparkle = if frame % 20 < 10 { 255 } else { 180 };
                        gpu::draw_rect_flat(screen_x + 8, screen_y + 2, 4, 4, sparkle, sparkle, 80);
                    }
                }
            }
        }
    }

    fn act_ground_color(&self, act: Act) -> (u8, u8, u8) {
        match act {
            Act::Act1Sanctuary => (50, 70, 45),    // Lush night grass
            Act::Act2Bushland => (140, 85, 45),    // Aussie red earth
            Act::Act3City => (80, 80, 85),        // Urban concrete & asphalt
            Act::Act4Ocean => (210, 180, 110),    // Golden beach sand
        }
    }

    fn draw_solid_tile(&self, act: Act, x: i16, y: i16, tx: i32, ty: i32) {
        let (r, g, b) = self.act_ground_color(act);
        gpu::draw_rect_flat(x, y, 16, 16, r, g, b);

        // Top edge grass / highlights
        if ty > 0 {
            match act {
                Act::Act1Sanctuary => {
                    gpu::draw_rect_flat(x, y, 16, 3, 70, 110, 55);
                }
                Act::Act2Bushland => {
                    gpu::draw_rect_flat(x, y, 16, 2, 180, 110, 60);
                }
                Act::Act3City => {
                    gpu::draw_rect_flat(x, y, 16, 2, 120, 120, 130);
                    // Brick line details
                    if (tx + ty) % 2 == 0 {
                        gpu::draw_rect_flat(x + 2, y + 6, 12, 1, 60, 60, 65);
                    }
                }
                Act::Act4Ocean => {
                    gpu::draw_rect_flat(x, y, 16, 2, 240, 215, 150);
                }
            }
        }
    }

    /// Render Platty the Platypus with detailed animation states!
    pub fn draw_platypus(&self, platty: &Platypus) {
        let px = (platty.x.to_int() - self.camera_x) as i16;
        let py = platty.y.to_int() as i16;

        // Invulnerability blink
        if platty.invuln_timer > 0 && (platty.invuln_timer / 4) % 2 == 1 {
            return;
        }

        let dir = platty.facing_right;
        let body_col = (95, 55, 30);      // Platypus brown fur
        let belly_col = (135, 85, 45);    // Lighter belly fur
        let bill_col = (45, 42, 42);      // Dark leathery duck bill
        let tail_col = (75, 42, 22);      // Beaver-like paddle tail
        let satchel_col = (175, 130, 65); // Little letter satchel
        let letter_col = (245, 245, 240); // Parents' precious white letter!

        match platty.state {
            PlayerState::BellySlide => {
                // Streamlined horizontal sliding pose
                let body_x = px;
                let body_y = py + 8;
                // Flat body
                gpu::draw_rect_flat(body_x + 2, body_y, 16, 6, body_col.0, body_col.1, body_col.2);
                gpu::draw_rect_flat(body_x + 3, body_y + 3, 14, 3, belly_col.0, belly_col.1, belly_col.2);

                if dir {
                    // Bill forward right
                    gpu::draw_rect_flat(body_x + 18, body_y + 2, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Tail trailing left
                    gpu::draw_rect_flat(body_x - 6, body_y + 1, 8, 4, tail_col.0, tail_col.1, tail_col.2);
                    // Eye
                    gpu::draw_rect_flat(body_x + 14, body_y + 1, 2, 2, 255, 255, 255);
                    gpu::draw_rect_flat(body_x + 15, body_y + 1, 1, 1, 0, 0, 0);
                    // Satchel strapped on back
                    gpu::draw_rect_flat(body_x + 8, body_y - 2, 5, 3, satchel_col.0, satchel_col.1, satchel_col.2);
                    gpu::draw_rect_flat(body_x + 9, body_y - 3, 3, 2, letter_col.0, letter_col.1, letter_col.2);
                } else {
                    // Bill forward left
                    gpu::draw_rect_flat(body_x - 4, body_y + 2, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Tail trailing right
                    gpu::draw_rect_flat(body_x + 18, body_y + 1, 8, 4, tail_col.0, tail_col.1, tail_col.2);
                    // Eye
                    gpu::draw_rect_flat(body_x + 4, body_y + 1, 2, 2, 255, 255, 255);
                    gpu::draw_rect_flat(body_x + 4, body_y + 1, 1, 1, 0, 0, 0);
                    // Satchel strapped on back
                    gpu::draw_rect_flat(body_x + 7, body_y - 2, 5, 3, satchel_col.0, satchel_col.1, satchel_col.2);
                    gpu::draw_rect_flat(body_x + 8, body_y - 3, 3, 2, letter_col.0, letter_col.1, letter_col.2);
                }
            }
            PlayerState::Swimming => {
                // Sleek aquatic swimming pose
                let body_x = px;
                let body_y = py + 4;
                gpu::draw_rect_flat(body_x + 2, body_y, 14, 8, body_col.0, body_col.1, body_col.2);
                gpu::draw_rect_flat(body_x + 4, body_y + 3, 10, 4, belly_col.0, belly_col.1, belly_col.2);

                let paddle_cycle = (platty.anim_frame / 4) % 2 == 0;
                let web_offset = if paddle_cycle { 1 } else { -1 };

                if dir {
                    // Bill
                    gpu::draw_rect_flat(body_x + 16, body_y + 2, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Paddle tail undulating
                    let tail_y = body_y + 1 + web_offset;
                    gpu::draw_rect_flat(body_x - 6, tail_y, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    // Webbed flipper
                    gpu::draw_rect_flat(body_x + 8, body_y + 7, 4, 3, 50, 45, 40);
                    // Eye
                    gpu::draw_rect_flat(body_x + 13, body_y + 1, 2, 2, 255, 255, 255);
                    // Satchel
                    gpu::draw_rect_flat(body_x + 6, body_y - 2, 5, 3, satchel_col.0, satchel_col.1, satchel_col.2);
                    gpu::draw_rect_flat(body_x + 7, body_y - 3, 3, 2, letter_col.0, letter_col.1, letter_col.2);
                } else {
                    // Bill
                    gpu::draw_rect_flat(body_x - 2, body_y + 2, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Paddle tail undulating
                    let tail_y = body_y + 1 + web_offset;
                    gpu::draw_rect_flat(body_x + 16, tail_y, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    // Webbed flipper
                    gpu::draw_rect_flat(body_x + 6, body_y + 7, 4, 3, 50, 45, 40);
                    // Eye
                    gpu::draw_rect_flat(body_x + 5, body_y + 1, 2, 2, 255, 255, 255);
                    // Satchel
                    gpu::draw_rect_flat(body_x + 9, body_y - 2, 5, 3, satchel_col.0, satchel_col.1, satchel_col.2);
                    gpu::draw_rect_flat(body_x + 10, body_y - 3, 3, 2, letter_col.0, letter_col.1, letter_col.2);
                }
            }
            _ => {
                // Standing, Running, Jumping, or TailWhip
                let body_x = px;
                let body_y = py;

                // Main body
                gpu::draw_rect_flat(body_x + 2, body_y + 4, 12, 10, body_col.0, body_col.1, body_col.2);
                gpu::draw_rect_flat(body_x + 4, body_y + 7, 8, 6, belly_col.0, belly_col.1, belly_col.2);

                // Satchel with the parents' letter!
                let satchel_x = if dir { body_x + 4 } else { body_x + 7 };
                gpu::draw_rect_flat(satchel_x, body_y + 4, 5, 5, satchel_col.0, satchel_col.1, satchel_col.2);
                gpu::draw_rect_flat(satchel_x + 1, body_y + 2, 3, 3, letter_col.0, letter_col.1, letter_col.2);

                if dir {
                    // Face right
                    // Duck bill
                    gpu::draw_rect_flat(body_x + 13, body_y + 5, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Eye
                    gpu::draw_rect_flat(body_x + 10, body_y + 4, 2, 2, 255, 255, 255);
                    gpu::draw_rect_flat(body_x + 11, body_y + 4, 1, 1, 0, 0, 0);

                    // Beaver tail
                    if platty.state == PlayerState::TailWhip {
                        // Swung forward dynamically!
                        gpu::draw_rect_flat(body_x + 18, body_y + 4, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                        // Swoosh trail
                        gpu::draw_line_mono(body_x + 20, body_y, body_x + 26, body_y + 8, 255, 255, 255);
                    } else if platty.state == PlayerState::Jumping {
                        // Tail tilted up
                        gpu::draw_rect_flat(body_x - 6, body_y + 1, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    } else {
                        // Tail resting / waddling
                        let tail_bob = if platty.state == PlayerState::Running && (platty.anim_frame / 4) % 2 == 0 { 1 } else { 0 };
                        gpu::draw_rect_flat(body_x - 6, body_y + 5 + tail_bob, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    }

                    // Webbed feet
                    let step = if platty.state == PlayerState::Running && (platty.anim_frame / 4) % 2 == 0 { 2 } else { 0 };
                    gpu::draw_rect_flat(body_x + 3, body_y + 14 - step, 4, 2, 50, 45, 40);
                    gpu::draw_rect_flat(body_x + 9, body_y + 14 + step / 2, 4, 2, 50, 45, 40);
                } else {
                    // Face left
                    // Duck bill
                    gpu::draw_rect_flat(body_x - 3, body_y + 5, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                    // Eye
                    gpu::draw_rect_flat(body_x + 4, body_y + 4, 2, 2, 255, 255, 255);
                    gpu::draw_rect_flat(body_x + 4, body_y + 4, 1, 1, 0, 0, 0);

                    // Beaver tail
                    if platty.state == PlayerState::TailWhip {
                        gpu::draw_rect_flat(body_x - 10, body_y + 4, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                        gpu::draw_line_mono(body_x - 12, body_y, body_x - 6, body_y + 8, 255, 255, 255);
                    } else if platty.state == PlayerState::Jumping {
                        gpu::draw_rect_flat(body_x + 14, body_y + 1, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    } else {
                        let tail_bob = if platty.state == PlayerState::Running && (platty.anim_frame / 4) % 2 == 0 { 1 } else { 0 };
                        gpu::draw_rect_flat(body_x + 14, body_y + 5 + tail_bob, 8, 6, tail_col.0, tail_col.1, tail_col.2);
                    }

                    // Webbed feet
                    let step = if platty.state == PlayerState::Running && (platty.anim_frame / 4) % 2 == 0 { 2 } else { 0 };
                    gpu::draw_rect_flat(body_x + 3, body_y + 14 + step / 2, 4, 2, 50, 45, 40);
                    gpu::draw_rect_flat(body_x + 9, body_y + 14 - step, 4, 2, 50, 45, 40);
                }
            }
        }

        // Electro-Reception (Bill Sense) wave effect!
        if platty.electro_timer > 0 {
            let wave_r = ((45 - platty.electro_timer) * 3) as i16;
            let center_x = if dir { px + 18 } else { px - 2 };
            let center_y = py + 7;

            // Draw expanding diamond radar pulse
            gpu::draw_line_mono(center_x, center_y - wave_r, center_x + wave_r, center_y, 100, 240, 255);
            gpu::draw_line_mono(center_x + wave_r, center_y, center_x, center_y + wave_r, 100, 240, 255);
            gpu::draw_line_mono(center_x, center_y + wave_r, center_x - wave_r, center_y, 100, 240, 255);
            gpu::draw_line_mono(center_x - wave_r, center_y, center_x, center_y - wave_r, 100, 240, 255);
        }
    }

    /// Render collectibles, enemies, and particles.
    pub fn draw_entities(&self, entities: &EntityManager, platty: &Platypus) {
        // Collectibles
        for c in entities.collectibles.iter() {
            if !c.active {
                continue;
            }
            let cx = (c.x - self.camera_x) as i16;
            if cx < -16 || cx > SCREEN_W {
                continue;
            }
            let bob = if (c.bob_timer / 10) % 2 == 0 { 1 } else { 0 };
            let cy = (c.y as i16) + bob;

            match c.kind {
                CollectibleType::Yabby => {
                    // Australian blue freshwater crayfish / yabby!
                    gpu::draw_rect_flat(cx + 4, cy + 4, 8, 6, 40, 120, 220); // body
                    gpu::draw_rect_flat(cx + 2, cy + 2, 3, 3, 30, 90, 180);  // left claw
                    gpu::draw_rect_flat(cx + 11, cy + 2, 3, 3, 30, 90, 180); // right claw
                    gpu::draw_rect_flat(cx + 6, cy + 10, 4, 4, 20, 70, 150); // tail fan

                    // If bill sense is active, highlight with golden pulse aura!
                    if platty.electro_timer > 0 {
                        gpu::draw_line_mono(cx, cy, cx + 16, cy + 16, 255, 240, 80);
                    }
                }
                CollectibleType::LetterPage => {
                    // Glowing letter scrap
                    gpu::draw_rect_flat(cx + 3, cy + 3, 10, 10, 255, 255, 245);
                    gpu::draw_rect_flat(cx + 6, cy + 6, 4, 4, 220, 160, 40); // gold seal
                }
            }
        }

        // Enemies
        for e in entities.enemies.iter() {
            if !e.active {
                continue;
            }
            let ex = (e.x.to_int() - self.camera_x) as i16;
            if ex < -32 || ex > SCREEN_W + 32 {
                continue;
            }
            let ey = e.y.to_int() as i16;

            match e.kind {
                EnemyType::Zookeeper => {
                    // Sanctuary Zookeeper in khaki uniform with flashlight cone
                    gpu::draw_rect_flat(ex + 4, ey, 8, 6, 230, 180, 140); // face
                    gpu::draw_rect_flat(ex + 2, ey - 3, 12, 3, 110, 95, 60); // brim hat
                    gpu::draw_rect_flat(ex + 2, ey + 6, 12, 10, 120, 105, 65); // khaki shirt

                    // Flashlight beam (blended yellow light cone)
                    let beam_dir = if e.facing_right { 1 } else { -1 };
                    let fx = ex + 8 + (beam_dir * 8);
                    let fy = ey + 8;
                    let cone_x = fx + (beam_dir * 50);

                    gpu::draw_tri_flat_blended(
                        [(fx, fy), (cone_x, fy - 18), (cone_x, fy + 18)],
                        180,
                        180,
                        60,
                        BlendMode::Add,
                    );
                }
                EnemyType::Wombat => {
                    // Cute stocky Australian wombat
                    gpu::draw_rect_flat(ex + 2, ey + 4, 14, 10, 115, 80, 50);
                    gpu::draw_rect_flat(ex + (if e.facing_right { 12 } else { 0 }), ey + 6, 4, 4, 50, 35, 20); // nose
                    gpu::draw_rect_flat(ex + 4, ey + 14, 3, 2, 70, 50, 30);
                    gpu::draw_rect_flat(ex + 11, ey + 14, 3, 2, 70, 50, 30);
                }
                EnemyType::Pigeon => {
                    // City rooftop pigeon
                    gpu::draw_rect_flat(ex + 4, ey + 4, 8, 6, 120, 130, 140);
                    gpu::draw_rect_flat(ex + (if e.facing_right { 10 } else { 2 }), ey + 5, 3, 2, 220, 160, 40); // beak
                    gpu::draw_rect_flat(ex + 2, ey + 2, 6, 3, 200, 200, 210); // wing
                }
                EnemyType::Crab => {
                    // Coastal beach crab
                    gpu::draw_rect_flat(ex + 3, ey + 6, 10, 8, 220, 70, 50);
                    gpu::draw_rect_flat(ex, ey + 3, 3, 4, 240, 90, 60);      // claw
                    gpu::draw_rect_flat(ex + 13, ey + 3, 3, 4, 240, 90, 60); // claw
                }
            }
        }

        // Particles
        for p in entities.particles.iter() {
            if !p.active {
                continue;
            }
            let px = (p.x - self.camera_x) as i16;
            let py = p.y as i16;
            if px >= 0 && px < SCREEN_W && py >= 0 && py < SCREEN_H {
                gpu::draw_rect_flat(px, py, p.size as u16, p.size as u16, p.color.0, p.color.1, p.color.2);
            }
        }
    }

    /// Render HUD overlay (Health hearts, Air meter, Score, Letter badge).
    pub fn draw_hud(&self, platty: &Platypus, act: Act) {
        // Top dark translucent ribbon
        gpu::draw_rect_flat(0, 0, 320, 20, 10, 15, 20);

        // Hearts (Health)
        for i in 0..platty.max_health {
            let hx = 8 + (i as i16 * 14);
            if i < platty.health {
                // Bright red heart
                gpu::draw_rect_flat(hx, 5, 10, 9, 230, 40, 50);
                gpu::draw_rect_flat(hx + 2, 3, 6, 2, 230, 40, 50);
            } else {
                // Empty heart outline
                gpu::draw_rect_flat(hx, 5, 10, 9, 70, 40, 45);
            }
        }

        // Yabbies counter icon & text
        gpu::draw_rect_flat(80, 5, 8, 8, 40, 130, 240);
        self.font.draw_text(92, 6, "YABBIES", (180, 220, 255));

        // Air meter when underwater
        if platty.in_water {
            gpu::draw_rect_flat(160, 6, 60, 7, 20, 40, 60);
            let air_w = (platty.air as u16 * 58) / 100;
            gpu::draw_rect_flat(161, 7, air_w, 5, 80, 220, 255);
        } else {
            // Act badge
            self.font.draw_text(160, 6, act.title(), (240, 220, 140));
        }

        // Letter pouch badge
        gpu::draw_rect_flat(275, 4, 12, 10, 250, 250, 240);
        gpu::draw_rect_flat(279, 7, 4, 4, 210, 150, 40);
        self.font.draw_text(290, 6, "OK", (100, 255, 120));
    }

    /// Render Title Screen.
    pub fn draw_title_screen(&self, frame: u8) {
        // Deep teal-cyan gradient
        gpu::draw_rect_flat(0, 0, 320, 120, 15, 30, 45);
        gpu::draw_rect_flat(0, 120, 320, 120, 25, 55, 75);

        // Logo banner
        gpu::draw_rect_flat(40, 35, 240, 55, 35, 75, 95);
        gpu::draw_rect_flat(42, 37, 236, 51, 15, 25, 35);

        self.font.draw_text(105, 45, "P L A T T Y P U S", (255, 220, 90));
        self.font.draw_text(80, 65, "JOURNEY HOME TO THE NATIVE WILD", (180, 230, 255));

        // Center platypus illustration
        let cx = 145;
        let cy = 115;
        gpu::draw_rect_flat(cx, cy, 30, 14, 95, 55, 30);
        gpu::draw_rect_flat(cx + 25, cy + 3, 12, 8, 45, 42, 42); // bill
        gpu::draw_rect_flat(cx - 14, cy + 2, 16, 10, 75, 42, 22); // tail
        gpu::draw_rect_flat(cx + 8, cy - 4, 10, 8, 175, 130, 65); // satchel
        gpu::draw_rect_flat(cx + 10, cy - 6, 6, 4, 255, 255, 255); // letter!

        // Blinking start prompt
        if (frame / 25) % 2 == 0 {
            self.font.draw_text(95, 160, "PRESS START OR CROSS", (255, 255, 255));
        }

        self.font.draw_text(70, 200, "PSX HOMEBREW  *  POWERED BY PSOXIDE", (130, 160, 180));
    }

    /// Render Parents' Letter Intro cutscene.
    pub fn draw_letter_intro(&self, _frame: u8) {
        // Darkened background
        gpu::draw_rect_flat(0, 0, 320, 240, 15, 20, 25);

        // Parchment letter box
        gpu::draw_rect_flat(25, 25, 270, 190, 245, 235, 210);
        gpu::draw_rect_flat(28, 28, 264, 184, 250, 245, 225);

        // Letter seal
        gpu::draw_rect_flat(145, 35, 30, 20, 180, 45, 40);
        self.font.draw_text(152, 40, "P&M", (255, 240, 200));

        // Letter text
        self.font.draw_text(40, 65, "DEAREST PLATTY,", (50, 40, 30));
        self.font.draw_text(40, 85, "WE HAVE THE MOST WONDERFUL NEWS!", (50, 40, 30));
        self.font.draw_text(40, 105, "YOU ARE GOING TO BE A BIG BROTHER!", (180, 40, 40));
        self.font.draw_text(40, 125, "AN EGG HAS LAID IN THE COASTAL BURROW.", (50, 40, 30));
        self.font.draw_text(40, 145, "PLEASE HURRY HOME AS FAST AS YOU CAN!", (50, 40, 30));
        self.font.draw_text(170, 170, "- LOVE, MOM & DAD", (50, 40, 30));

        self.font.draw_text(75, 195, "PRESS CROSS TO SNEAK OUT!", (40, 110, 180));
    }

    /// Render Stage Clear screen.
    pub fn draw_stage_clear(&self, act: Act, _score: u32, _yabbies: u16) {
        gpu::draw_rect_flat(30, 40, 260, 160, 20, 35, 55);
        gpu::draw_rect_flat(33, 43, 254, 154, 10, 20, 30);

        self.font.draw_text(90, 60, "STAGE COMPLETE!", (100, 255, 150));
        self.font.draw_text(60, 85, act.title(), (255, 220, 90));

        self.font.draw_text(60, 115, "YABBIES ENJOYED:  ", (180, 220, 255));
        self.font.draw_text(60, 135, "JOURNEY DISTANCE: 100% CLEAR", (180, 220, 255));

        self.font.draw_text(75, 170, "PRESS CROSS TO CONTINUE", (255, 255, 255));
    }

    /// Render Family Reunion Ending cutscene!
    pub fn draw_ending(&self, frame: u8) {
        // Coastal sunset / golden beach
        gpu::draw_rect_flat(0, 0, 320, 120, 240, 160, 100);
        gpu::draw_rect_flat(0, 120, 320, 120, 210, 180, 120);

        // Estuary river water
        gpu::draw_rect_flat(0, 180, 320, 60, 50, 120, 170);

        // Family Burrow
        gpu::draw_rect_flat(120, 130, 80, 40, 110, 75, 45);
        gpu::draw_rect_flat(135, 140, 50, 30, 30, 20, 15);

        // Mom & Dad Platypus
        gpu::draw_rect_flat(90, 145, 24, 12, 95, 55, 30); // Mom
        gpu::draw_rect_flat(82, 148, 9, 6, 45, 42, 42); // Bill
        gpu::draw_rect_flat(205, 145, 24, 12, 95, 55, 30); // Dad
        gpu::draw_rect_flat(228, 148, 9, 6, 45, 42, 42); // Bill

        // Platty in center
        gpu::draw_rect_flat(148, 145, 20, 10, 95, 55, 30);
        // The tiny hatched egg / baby puggle!
        let egg_bob = if (frame / 10) % 2 == 0 { 1 } else { 0 };
        gpu::draw_rect_flat(152, 134 - egg_bob, 12, 10, 255, 250, 220); // Egg shell
        gpu::draw_rect_flat(155, 136 - egg_bob, 6, 5, 135, 95, 60); // Tiny baby puggle head!

        // Dialogue ribbon
        gpu::draw_rect_flat(20, 20, 280, 70, 20, 30, 40);
        gpu::draw_rect_flat(22, 22, 276, 66, 10, 15, 20);

        self.font.draw_text(40, 30, "MOM & DAD: PLATTY! YOU MADE IT HOME!", (255, 230, 100));
        self.font.draw_text(40, 50, "MEET YOUR NEW BABY SISTER PIP!", (100, 255, 180));
        self.font.draw_text(40, 70, "CONGRATULATIONS, BIG BROTHER!", (255, 255, 255));
    }
}
