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

/// Draw an axis-aligned Gouraud shaded rectangle using two textured/flat triangles.
#[inline]
pub fn draw_gradient_rect(x0: i16, y0: i16, x1: i16, y1: i16, c_top: (u8, u8, u8), c_bottom: (u8, u8, u8)) {
    gpu::draw_tri_gouraud([(x0, y0), (x1, y0), (x0, y1)], [c_top, c_top, c_bottom]);
    gpu::draw_tri_gouraud([(x1, y0), (x1, y1), (x0, y1)], [c_top, c_bottom, c_bottom]);
}

pub struct Renderer {
    pub fb: FrameBuffer,
    pub font: FontAtlas,
    pub camera_x: i32,
    pub camera_y: i32,
    pub screen_shake: i16,
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
            camera_y: 0,
            screen_shake: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        psx_rt::interrupts::wait_vblank();
        self.fb.swap();
    }

    pub fn update_camera(&mut self, player_x: Fixed, player_y: Fixed) {
        let px = player_x.to_int();
        let py = player_y.to_int();

        // Horizontal camera centering
        let target_x = px - (SCREEN_W as i32 / 2);
        let max_cam_x = (LEVEL_W as i32 * TILE_SIZE) - SCREEN_W as i32;
        if target_x < 0 {
            self.camera_x = 0;
        } else if target_x > max_cam_x {
            self.camera_x = max_cam_x;
        } else {
            self.camera_x = target_x;
        }

        // Vertical camera centering
        let target_y = py - (SCREEN_H as i32 / 2);
        let max_cam_y = (LEVEL_H as i32 * TILE_SIZE) - SCREEN_H as i32;
        if target_y < 0 {
            self.camera_y = 0;
        } else if target_y > max_cam_y {
            self.camera_y = max_cam_y;
        } else {
            self.camera_y = target_y;
        }

        // Dynamic screen shake impact offset
        if self.screen_shake > 0 {
            let shake = if (self.screen_shake & 1) != 0 { self.screen_shake } else { -self.screen_shake };
            self.camera_x = (self.camera_x + shake as i32).clamp(0, max_cam_x);
            self.camera_y = (self.camera_y - (shake as i32 / 2)).clamp(0, max_cam_y);
            self.screen_shake -= 1;
        }
    }

    /// Render sky and multi-layer parallax background according to current Act.
    pub fn draw_background(&self, act: Act, frame: u8) {
        let cam_y = (self.camera_y / 6) as i16;

        match act {
            Act::Act1Sanctuary => {
                // Gouraud Night Sky: Deep Midnight Navy -> Twilight Indigo -> River Mist
                draw_gradient_rect(0, 0, 320, 130, (4, 6, 20), (22, 28, 64));
                draw_gradient_rect(0, 130, 320, 240, (22, 28, 64), (12, 18, 40));

                // Moon & Glowing Halo
                gpu::draw_rect_flat(248, 22, 34, 34, 35, 45, 80);
                gpu::draw_rect_flat(251, 25, 28, 28, 70, 85, 125);
                gpu::draw_rect_flat(254, 28, 22, 22, 245, 245, 225);
                gpu::draw_rect_flat(252, 30, 26, 18, 245, 245, 225);
                // Moon craters
                gpu::draw_rect_flat(258, 33, 4, 3, 210, 210, 195);
                gpu::draw_rect_flat(265, 37, 5, 4, 210, 210, 195);

                // Twinkling Starfield with subtle horizontal parallax
                let star_drift = (self.camera_x / 16) as i16;
                let stars: [(i16, i16, u8); 8] = [
                    (35, 25, 1), (75, 42, 2), (120, 18, 0), (165, 55, 3),
                    (205, 30, 1), (235, 70, 2), (285, 20, 0), (310, 60, 3),
                ];
                for (sx, sy, phase) in stars.iter() {
                    let x = (*sx - star_drift).rem_euclid(330) - 5;
                    let y = *sy - cam_y / 2;
                    let b = if ((frame / 16) + phase) % 4 == 0 { 255 } else if ((frame / 16) + phase) % 4 == 1 { 190 } else { 120 };
                    gpu::draw_rect_flat(x, y, 2, 2, b, b, b.saturating_add(20));
                }

                // Layer 1: Distant Great Dividing Range mountain ridges (cam / 10)
                let mtn_offset = (self.camera_x / 10) as i16;
                for i in 0..5 {
                    let mx = (i * 90 - mtn_offset).rem_euclid(450) - 60;
                    let my = 155 - cam_y;
                    gpu::draw_tri_gouraud(
                        [(mx, my), (mx + 50, my - 45), (mx + 100, my)],
                        [(14, 20, 42), (26, 36, 70), (14, 20, 42)],
                    );
                }

                // Layer 2: Midground Gum tree silhouettes with moonlit rims (cam / 5)
                let tree_offset = (self.camera_x / 5) as i16;
                for i in 0..6 {
                    let tx = (i * 65 - tree_offset).rem_euclid(390) - 30;
                    let ty = 175 - (cam_y * 4 / 3);
                    // Tree Trunk
                    gpu::draw_rect_flat(tx + 12, ty - 45, 7, 50, 16, 24, 32);
                    // Foliage Canopy
                    gpu::draw_rect_flat(tx - 4, ty - 60, 38, 25, 12, 20, 26);
                    gpu::draw_rect_flat(tx + 2, ty - 75, 26, 20, 18, 28, 36);
                    // Moonlit highlight on canopy top
                    gpu::draw_rect_flat(tx + 4, ty - 76, 22, 2, 35, 55, 75);
                }

                // Layer 3: Riverbank mist & bulrushes (cam / 3)
                let reed_offset = (self.camera_x / 3) as i16;
                for i in 0..8 {
                    let rx = (i * 45 - reed_offset).rem_euclid(360) - 20;
                    let ry = 200 - cam_y * 2;
                    gpu::draw_rect_flat(rx, ry - 18, 2, 22, 18, 30, 25);
                    gpu::draw_rect_flat(rx - 1, ry - 24, 4, 7, 35, 22, 14); // Bulrush head
                }

                // Layer 4: Ambient glowing fireflies drifting lazily
                for f in 0..5 {
                    let fx = (f * 72 - (self.camera_x / 2) as i16 + (frame as i16 * 2)).rem_euclid(350) - 15;
                    let fy = 145 + ((f * 19 + frame as i16) % 35) - cam_y * 2;
                    let glow = if (frame + f as u8 * 13) % 20 < 10 { 255 } else { 130 };
                    gpu::draw_rect_flat(fx, fy, 2, 2, glow, 255, 110);
                }
            }
            Act::Act2Bushland => {
                // Gouraud Dawn Sky: Mauve Violet -> Fiery Crimson/Amber -> Golden Yellow -> Red Earth Haze
                draw_gradient_rect(0, 0, 320, 75, (55, 25, 70), (195, 75, 45));
                draw_gradient_rect(0, 75, 320, 150, (195, 75, 45), (245, 180, 75));
                draw_gradient_rect(0, 150, 320, 240, (245, 180, 75), (170, 105, 55));

                // Rising morning sun disk with corona rays
                let sun_y = 85 - cam_y / 2;
                gpu::draw_rect_flat(170, sun_y - 12, 36, 36, 255, 235, 155);
                gpu::draw_rect_flat(174, sun_y - 8, 28, 28, 255, 255, 220);
                gpu::draw_rect_flat(155, sun_y + 4, 66, 4, 255, 220, 120);

                // Layer 1: Flinders Ranges red escarpments (cam / 10)
                let hill_offset = (self.camera_x / 10) as i16;
                for i in 0..5 {
                    let hx = (i * 95 - hill_offset).rem_euclid(475) - 60;
                    let hy = 160 - cam_y;
                    gpu::draw_tri_gouraud(
                        [(hx, hy), (hx + 45, hy - 42), (hx + 105, hy)],
                        [(110, 55, 45), (160, 80, 55), (110, 55, 45)],
                    );
                }

                // Layer 2: Ghost Gums with silver trunks and olive crowns (cam / 5)
                let gum_offset = (self.camera_x / 5) as i16;
                for i in 0..6 {
                    let gx = (i * 70 - gum_offset).rem_euclid(420) - 30;
                    let gy = 180 - (cam_y * 4 / 3);
                    // Pale silver trunk
                    gpu::draw_rect_flat(gx + 10, gy - 50, 6, 52, 205, 200, 190);
                    gpu::draw_rect_flat(gx + 6, gy - 38, 14, 4, 185, 180, 170); // Gnarled branch
                    // Olive foliage crowns
                    gpu::draw_rect_flat(gx - 4, gy - 62, 34, 18, 75, 90, 48);
                    gpu::draw_rect_flat(gx + 4, gy - 72, 24, 14, 90, 110, 58);
                }

                // Layer 3: Cathedral termite mounds & scrub (cam / 3)
                let scrub_offset = (self.camera_x / 3) as i16;
                for i in 0..7 {
                    let sx = (i * 55 - scrub_offset).rem_euclid(385) - 25;
                    let sy = 205 - cam_y * 2;
                    // Pointed terracotta termite mound
                    gpu::draw_tri_flat([(sx, sy), (sx + 8, sy - 28), (sx + 16, sy)], 135, 70, 38);
                    // Spinifex grass clump
                    gpu::draw_rect_flat(sx + 20, sy - 8, 12, 8, 180, 150, 65);
                }

                // Layer 4: Drifting golden outback dust motes
                for d in 0..6 {
                    let dx = (d * 60 - (self.camera_x / 2) as i16 + (frame as i16)).rem_euclid(340) - 10;
                    let dy = 130 + ((d * 23 + frame as i16 / 2) % 40) - cam_y * 2;
                    gpu::draw_rect_flat(dx, dy, 2, 2, 255, 220, 140);
                }
            }
            Act::Act3City => {
                // Gouraud Urban Smog: Industrial Slate -> Cyan Haze -> Streetlamp Underglow
                draw_gradient_rect(0, 0, 320, 120, (12, 16, 24), (20, 48, 62));
                draw_gradient_rect(0, 120, 320, 240, (20, 48, 62), (48, 38, 32));

                // Layer 1: Distant Skyscraper Silhouettes & Towers (cam / 10)
                let bldg_far_offset = (self.camera_x / 10) as i16;
                for i in 0..6 {
                    let bx = (i * 65 - bldg_far_offset).rem_euclid(390) - 30;
                    let bh = 90 + (i * 17 % 45);
                    let by = 170 - bh - cam_y;
                    gpu::draw_rect_flat(bx, by, 32, bh as u16, 14, 18, 24);
                    // Antenna mast with blinking red beacon
                    gpu::draw_rect_flat(bx + 15, by - 12, 2, 12, 40, 45, 55);
                    let beacon_red = if (frame / 20 + i as u8) % 2 == 0 { 255 } else { 60 };
                    gpu::draw_rect_flat(bx + 14, by - 14, 4, 3, beacon_red, 30, 30);
                }

                // Layer 2: Midground illuminated office blocks & factories (cam / 5)
                let bldg_mid_offset = (self.camera_x / 5) as i16;
                for i in 0..7 {
                    let bx = (i * 55 - bldg_mid_offset).rem_euclid(385) - 30;
                    let bh = 75 + (i * 13 % 35);
                    let by = 185 - bh - (cam_y * 4 / 3);
                    gpu::draw_rect_flat(bx, by, 38, bh as u16, 22, 26, 32);
                    // Lighted window grids
                    for wy in 0..4 {
                        let win_y = by + 10 + wy * 12;
                        if i % 2 == 0 {
                            gpu::draw_rect_flat(bx + 8, win_y, 4, 6, 240, 205, 95);   // Warm amber office
                            gpu::draw_rect_flat(bx + 22, win_y, 4, 6, 95, 220, 245);  // Cool cyan terminal
                        } else {
                            gpu::draw_rect_flat(bx + 14, win_y, 6, 5, 245, 220, 140);
                        }
                    }
                }

                // Layer 3: Overhead utility poles, wires & truss bridges (cam / 3)
                let wire_y = 100 - cam_y * 2;
                gpu::draw_line_mono(0, wire_y, 320, wire_y + 8, 42, 48, 55);
                gpu::draw_line_mono(0, wire_y + 12, 320, wire_y + 20, 42, 48, 55);

                // Layer 4: Neon billboard signs with pulsating glow
                let sign_glow = if (frame / 12) % 2 == 0 { 255 } else { 180 };
                gpu::draw_rect_flat(180 - (self.camera_x / 4) as i16, 75 - cam_y, 32, 10, 20, 15, 30);
                gpu::draw_rect_flat(182 - (self.camera_x / 4) as i16, 77 - cam_y, 28, 6, sign_glow, 60, 210);
            }
            Act::Act4Ocean => {
                // Gouraud Coastal Sunrise: Coral Rose -> Radiant Gold -> Turquoise Waves -> Sapphire Abyss
                draw_gradient_rect(0, 0, 320, 85, (220, 80, 85), (255, 175, 100));
                draw_gradient_rect(0, 85, 320, 135, (255, 175, 100), (35, 135, 165));
                draw_gradient_rect(0, 135, 320, 240, (35, 135, 165), (15, 55, 105));

                // Blazing Sunrise on the Horizon
                let sun_y = 115 - cam_y / 2;
                gpu::draw_rect_flat(142, sun_y - 18, 36, 36, 255, 240, 180);
                // Vertical shimmering sun reflection column down into the ocean
                for ry in 0..8 {
                    let rw = 28 - ry * 3;
                    let rx = 160 - rw / 2;
                    let y = 135 + ry * 8 - cam_y;
                    let sh = if (frame / 4 + ry as u8) % 2 == 0 { 255 } else { 190 };
                    gpu::draw_rect_flat(rx, y, rw as u16, 2, sh, 215, 130);
                }

                // Layer 1: Distant Headland Sea Cliffs & Lighthouse (cam / 12)
                let cliff_offset = (self.camera_x / 12) as i16;
                let cx = (260 - cliff_offset).rem_euclid(380) - 40;
                let cy = 135 - cam_y;
                gpu::draw_tri_flat([(cx, cy), (cx + 35, cy - 30), (cx + 70, cy)], 45, 65, 80);
                // White lighthouse tower
                gpu::draw_rect_flat(cx + 32, cy - 42, 6, 12, 245, 245, 240);
                gpu::draw_rect_flat(cx + 31, cy - 45, 8, 4, 180, 45, 45); // Red lantern room
                // Flashing beacon beam
                if (frame / 15) % 2 == 0 {
                    gpu::draw_tri_flat([(cx + 35, cy - 43), (cx - 30, cy - 55), (cx - 30, cy - 35)], 255, 255, 180);
                }

                // Layer 2: Rolling ocean swell waves with seafoam crests (cam / 5)
                let swell_offset = ((self.camera_x / 5) as i16 + (frame / 2) as i16) % 40;
                for i in 0..10 {
                    let wx = i * 40 - swell_offset;
                    let wy = 150 - (cam_y * 4 / 3);
                    gpu::draw_rect_flat(wx, wy, 24, 3, 55, 160, 195);
                    gpu::draw_rect_flat(wx + 4, wy - 1, 16, 1, 230, 250, 255); // Whitecap
                }

                // Layer 3: Pier Pilings & coastal paperbarks (cam / 3)
                let pier_offset = (self.camera_x / 3) as i16;
                for i in 0..6 {
                    let px = (i * 65 - pier_offset).rem_euclid(390) - 20;
                    let py = 185 - cam_y * 2;
                    gpu::draw_rect_flat(px, py - 20, 6, 35, 85, 60, 40); // Weathered wooden pile
                    gpu::draw_rect_flat(px - 1, py - 4, 8, 2, 70, 140, 65); // Green sea barnacles
                }

                // Layer 4: Soaring seagulls gliding across the dawn
                for g in 0..3 {
                    let gx = (g * 115 - (self.camera_x / 3) as i16 + (frame as i16)).rem_euclid(350) - 20;
                    let gy = 55 + g * 22 + if (frame / 10 + g as u8) % 2 == 0 { -2 } else { 2 } - cam_y;
                    let wing_y = gy + if (frame / 8 + g as u8) % 2 == 0 { -3 } else { 2 };
                    gpu::draw_line_mono(gx, wing_y, gx + 6, gy, 255, 255, 255);
                    gpu::draw_line_mono(gx + 6, gy, gx + 12, wing_y, 255, 255, 255);
                }
            }
        }
    }

    /// Render visible level tiles within camera frustum.
    pub fn draw_level(&self, level: &Level, frame: u8) {
        let start_col = (self.camera_x / TILE_SIZE).max(0);
        let end_col = ((self.camera_x + SCREEN_W as i32) / TILE_SIZE + 1).min(LEVEL_W as i32);
        let start_row = (self.camera_y / TILE_SIZE).max(0);
        let end_row = ((self.camera_y + SCREEN_H as i32) / TILE_SIZE + 1).min(LEVEL_H as i32);

        for tx in start_col..end_col {
            let screen_x = (tx * TILE_SIZE - self.camera_x) as i16;

            for ty in start_row..end_row {
                let screen_y = (ty * TILE_SIZE - self.camera_y) as i16;
                let tile = level.get_tile(tx, ty);

                match tile {
                    TileType::Empty => {}
                    TileType::Solid => {
                        self.draw_solid_tile(level, level.act, screen_x, screen_y, tx, ty);
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
                        // Highlight edge along slope diagonal
                        let hi_r = c.0.saturating_add(45);
                        let hi_g = c.1.saturating_add(45);
                        let hi_b = c.2.saturating_add(45);
                        gpu::draw_line_mono(screen_x, screen_y, screen_x + 16, screen_y + 16, hi_r, hi_g, hi_b);
                    }
                    TileType::Platform => {
                        // Polished wooden plank with 3D bevel and support bracket
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 4, 155, 110, 65);
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 1, 195, 150, 95); // Light top edge
                        gpu::draw_rect_flat(screen_x, screen_y + 3, 16, 1, 95, 65, 35); // Dark bottom edge
                        gpu::draw_rect_flat(screen_x + 4, screen_y + 4, 2, 6, 105, 75, 45); // Support bracket
                        gpu::draw_rect_flat(screen_x + 10, screen_y + 4, 2, 6, 105, 75, 45);
                    }
                    TileType::Water => {
                        let surface = ty == 0 || level.get_tile(tx, ty - 1) != TileType::Water;
                        if surface {
                            // Animated water surface ripple with specular foam
                            let ripple = if (frame / 8 + tx as u8) % 2 == 0 { 2 } else { 0 };
                            gpu::draw_rect_flat(screen_x, screen_y + ripple, 16, (16 - ripple) as u16, 25, 85, 165);
                            // Surface wave highlight
                            gpu::draw_rect_flat(screen_x, screen_y + ripple, 16, 2, 110, 195, 255);
                            let shimmer_x = screen_x + (((frame as i16 * 2) + (tx as i16 * 5)) % 13);
                            gpu::draw_rect_flat(shimmer_x, screen_y + ripple + 1, 3, 1, 255, 255, 255);
                        } else {
                            // Submerged deep water body
                            gpu::draw_rect_flat(screen_x, screen_y, 16, 16, 18, 65, 135);
                            // Subsurface current flow
                            if (tx + ty) % 3 == 0 {
                                let flow = ((frame / 6) + ty as u8) % 4;
                                gpu::draw_rect_flat(screen_x + 3, screen_y + 4 + flow as i16, 10, 1, 35, 95, 175);
                            }
                            // Occasional rising bubble
                            if (tx * 11 + ty * 7) % 7 == 0 {
                                let by = screen_y + (16 - ((frame as i16 * 2 + tx as i16 * 3) % 16));
                                gpu::draw_rect_flat(screen_x + 7, by, 2, 2, 160, 230, 255);
                            }
                        }
                    }
                    TileType::Hazard => {
                        // Spikes with metallic sheen & red tip
                        gpu::draw_tri_flat(
                            [(screen_x, screen_y + 16), (screen_x + 8, screen_y + 3), (screen_x + 16, screen_y + 16)],
                            190, 50, 45,
                        );
                        // Gleaming tip
                        gpu::draw_tri_flat(
                            [(screen_x + 5, screen_y + 8), (screen_x + 8, screen_y + 3), (screen_x + 11, screen_y + 8)],
                            255, 230, 210,
                        );
                        // Dark iron base
                        gpu::draw_rect_flat(screen_x + 1, screen_y + 14, 14, 2, 60, 40, 45);
                    }
                    TileType::Exit => {
                        // Glowing transition archway / burrow entrance
                        gpu::draw_rect_flat(screen_x - 2, screen_y - 8, 20, 24, 60, 40, 25);
                        gpu::draw_rect_flat(screen_x + 2, screen_y - 4, 12, 20, 20, 15, 10);
                        // Golden goal sparkle
                        let sparkle = if frame % 20 < 10 { 255 } else { 180 };
                        gpu::draw_rect_flat(screen_x + 6, screen_y + 2, 4, 4, sparkle, sparkle, 80);
                    }
                    TileType::BouncyPad => {
                        // Springy lily pad / bouncy mushroom cushion with animated pulse
                        gpu::draw_rect_flat(screen_x + 6, screen_y + 10, 4, 6, 45, 105, 45); // stalk
                        gpu::draw_rect_flat(screen_x + 7, screen_y + 11, 2, 4, 65, 140, 65);
                        let bounce_pulse = if (frame / 10) % 2 == 0 { 1 } else { 0 };
                        gpu::draw_rect_flat(screen_x + 1, screen_y + 5 - bounce_pulse, 14, 6, 60, 195, 85); // pad
                        gpu::draw_rect_flat(screen_x + 3, screen_y + 3 - bounce_pulse, 10, 4, 130, 250, 125); // bouncy top
                        gpu::draw_rect_flat(screen_x + 5, screen_y + 2 - bounce_pulse, 6, 2, 220, 255, 190); // highlight
                        gpu::draw_rect_flat(screen_x + 4, screen_y + 5 - bounce_pulse, 2, 2, 255, 240, 140); // spots
                        gpu::draw_rect_flat(screen_x + 10, screen_y + 5 - bounce_pulse, 2, 2, 255, 240, 140);
                    }
                    TileType::BreakableMud => {
                        // Cracked mud / breakable block with 3D bevel and fissure cracks
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 16, 130, 85, 50);
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 1, 165, 115, 75);
                        gpu::draw_rect_flat(screen_x, screen_y + 15, 16, 1, 85, 50, 30);
                        // Fissure cracks
                        gpu::draw_line_mono(screen_x + 2, screen_y + 3, screen_x + 8, screen_y + 8, 55, 30, 18);
                        gpu::draw_line_mono(screen_x + 8, screen_y + 8, screen_x + 14, screen_y + 13, 55, 30, 18);
                        gpu::draw_line_mono(screen_x + 8, screen_y + 8, screen_x + 4, screen_y + 14, 55, 30, 18);
                        // Pebble fragments
                        gpu::draw_rect_flat(screen_x + 11, screen_y + 3, 3, 2, 175, 130, 85);
                        gpu::draw_rect_flat(screen_x + 4, screen_y + 10, 2, 2, 175, 130, 85);
                    }
                    TileType::WaterCurrentRight => {
                        // Churning flume rushing right with animated foam
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 16, 28, 105, 185);
                        let flow_x = screen_x + (((frame as i16 * 2) + (tx as i16 * 4)) % 16);
                        gpu::draw_tri_flat(
                            [(flow_x, screen_y + 3), (flow_x + 5, screen_y + 8), (flow_x, screen_y + 13)],
                            190, 235, 255,
                        );
                        gpu::draw_rect_flat(screen_x, screen_y + 1, 16, 1, 140, 215, 255);
                    }
                    TileType::WaterCurrentLeft => {
                        // Churning flume rushing left
                        gpu::draw_rect_flat(screen_x, screen_y, 16, 16, 28, 105, 185);
                        let flow_x = screen_x + 16 - (((frame as i16 * 2) + (tx as i16 * 4)) % 16);
                        gpu::draw_tri_flat(
                            [(flow_x, screen_y + 3), (flow_x - 5, screen_y + 8), (flow_x, screen_y + 13)],
                            190, 235, 255,
                        );
                        gpu::draw_rect_flat(screen_x, screen_y + 1, 16, 1, 140, 215, 255);
                    }
                    TileType::FloatingLog => {
                        // Floating wooden log on water with moss and bark grain
                        gpu::draw_rect_flat(screen_x, screen_y + 6, 16, 10, 25, 85, 165); // water beneath
                        gpu::draw_rect_flat(screen_x, screen_y + 3, 16, 7, 125, 80, 40); // log body
                        gpu::draw_rect_flat(screen_x, screen_y + 3, 16, 1, 160, 110, 60); // top light
                        gpu::draw_rect_flat(screen_x + 3, screen_y + 5, 2, 4, 80, 50, 20); // bark grain
                        gpu::draw_rect_flat(screen_x + 11, screen_y + 5, 2, 4, 80, 50, 20);
                        gpu::draw_rect_flat(screen_x, screen_y + 2, 16, 2, 75, 150, 60); // moss top
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

    fn draw_solid_tile(&self, level: &Level, act: Act, x: i16, y: i16, tx: i32, ty: i32) {
        let (r, g, b) = self.act_ground_color(act);
        gpu::draw_rect_flat(x, y, 16, 16, r, g, b);

        // 3D bevel relief (Top & Left bright highlight, Bottom & Right deep shadow)
        let hi_r = r.saturating_add(30);
        let hi_g = g.saturating_add(30);
        let hi_b = b.saturating_add(30);
        let lo_r = r.saturating_sub(35);
        let lo_g = g.saturating_sub(35);
        let lo_b = b.saturating_sub(35);

        gpu::draw_rect_flat(x, y, 16, 1, hi_r, hi_g, hi_b);
        gpu::draw_rect_flat(x, y, 1, 16, hi_r, hi_g, hi_b);
        gpu::draw_rect_flat(x, y + 15, 16, 1, lo_r, lo_g, lo_b);
        gpu::draw_rect_flat(x + 15, y, 1, 16, lo_r, lo_g, lo_b);

        let above_empty = ty == 0 || level.get_tile(tx, ty - 1) != TileType::Solid;

        if above_empty {
            // Surface edge decoration
            match act {
                Act::Act1Sanctuary => {
                    // Lush mossy grass band + hanging grass tufts
                    gpu::draw_rect_flat(x, y, 16, 3, 75, 135, 55);
                    gpu::draw_rect_flat(x + 1, y, 14, 1, 110, 180, 80);
                    if (tx & 1) == 0 {
                        gpu::draw_rect_flat(x + 3, y + 3, 2, 2, 60, 115, 45);
                        gpu::draw_rect_flat(x + 10, y + 3, 3, 3, 60, 115, 45);
                    } else {
                        gpu::draw_rect_flat(x + 6, y + 3, 3, 2, 60, 115, 45);
                        gpu::draw_rect_flat(x + 13, y + 3, 2, 3, 60, 115, 45);
                    }
                }
                Act::Act2Bushland => {
                    // Golden sun-baked grass crest + red dirt striations
                    gpu::draw_rect_flat(x, y, 16, 2, 205, 165, 70);
                    gpu::draw_rect_flat(x + 2, y + 5, 11, 2, 175, 100, 55);
                    if (tx + ty) % 3 == 0 {
                        gpu::draw_rect_flat(x + 4, y + 10, 3, 2, 90, 50, 25); // Pebble
                    }
                }
                Act::Act3City => {
                    // Concrete coping slab + steel expansion joint
                    gpu::draw_rect_flat(x, y, 16, 3, 140, 145, 155);
                    gpu::draw_rect_flat(x, y + 3, 16, 1, 50, 50, 55); // Mortar groove
                    if (tx + ty) % 2 == 0 {
                        gpu::draw_rect_flat(x + 7, y + 8, 2, 2, 180, 190, 205); // Steel rivet
                    }
                }
                Act::Act4Ocean => {
                    // Warm sand dune crest with shell flecks
                    gpu::draw_rect_flat(x, y, 16, 3, 245, 225, 160);
                    if (tx * 7 + ty * 3) % 4 == 0 {
                        gpu::draw_rect_flat(x + 4, y + 6, 2, 2, 255, 255, 240); // Shell
                    }
                    if (tx + ty) % 2 == 1 {
                        gpu::draw_rect_flat(x + 10, y + 9, 2, 2, 180, 140, 80); // Coral fleck
                    }
                }
            }
        } else {
            // Subterranean underground texture
            match act {
                Act::Act1Sanctuary => {
                    if (tx * 3 + ty * 5) % 5 == 0 {
                        gpu::draw_rect_flat(x + 5, y + 6, 4, 3, 35, 50, 32); // Dark earth pocket
                    }
                }
                Act::Act2Bushland => {
                    if (tx * 4 + ty * 2) % 4 == 0 {
                        gpu::draw_rect_flat(x + 3, y + 7, 6, 2, 165, 95, 50); // Red rock layer
                    }
                }
                Act::Act3City => {
                    let course_y = if (ty & 1) == 0 { 6 } else { 10 };
                    gpu::draw_rect_flat(x, y + course_y, 16, 1, 55, 55, 60); // Brick course
                }
                Act::Act4Ocean => {
                    if (tx + ty * 2) % 3 == 0 {
                        gpu::draw_rect_flat(x + 4, y + 5, 5, 4, 185, 155, 95); // Wet sandstone
                    }
                }
            }
        }
    }

    /// Render Platty the Platypus with detailed animation states!
    pub fn draw_platypus(&self, platty: &Platypus) {
        let px = (platty.x.to_int() - self.camera_x) as i16;
        let py = (platty.y.to_int() - self.camera_y) as i16;

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
            PlayerState::SpurStomp => {
                // Venomous spur stomp dive - curled offensive sphere with venom spikes
                let body_x = px;
                let body_y = py;
                // Curled body
                gpu::draw_rect_flat(body_x + 2, body_y + 2, 12, 10, body_col.0, body_col.1, body_col.2);
                gpu::draw_rect_flat(body_x + 4, body_y + 3, 8, 6, belly_col.0, belly_col.1, belly_col.2);

                // Tail curled upward
                gpu::draw_rect_flat(body_x + 4, body_y - 4, 8, 6, tail_col.0, tail_col.1, tail_col.2);

                // Bill tucked down
                gpu::draw_rect_flat(body_x + 5, body_y + 8, 6, 4, bill_col.0, bill_col.1, bill_col.2);
                // Eyes focused down
                gpu::draw_rect_flat(body_x + 4, body_y + 5, 2, 2, 255, 255, 255);
                gpu::draw_rect_flat(body_x + 10, body_y + 5, 2, 2, 255, 255, 255);

                // Venomous spur spikes extended downward from hind feet!
                gpu::draw_rect_flat(body_x + 2, body_y + 12, 3, 5, 220, 80, 255);
                gpu::draw_rect_flat(body_x + 11, body_y + 12, 3, 5, 220, 80, 255);
                gpu::draw_rect_flat(body_x + 3, body_y + 15, 1, 3, 255, 220, 255);
                gpu::draw_rect_flat(body_x + 12, body_y + 15, 1, 3, 255, 220, 255);

                // Satchel
                gpu::draw_rect_flat(body_x + 5, body_y + 3, 6, 4, satchel_col.0, satchel_col.1, satchel_col.2);
            }
            PlayerState::WallSlide => {
                let body_x = px;
                let body_y = py;
                if platty.wall_slide_side == -1 {
                    // Left wall: tail pressed flat against wall
                    gpu::draw_rect_flat(body_x, body_y + 2, 4, 12, tail_col.0, tail_col.1, tail_col.2);
                    gpu::draw_rect_flat(body_x + 4, body_y + 3, 10, 10, body_col.0, body_col.1, body_col.2);
                    gpu::draw_rect_flat(body_x + 6, body_y + 5, 7, 6, belly_col.0, belly_col.1, belly_col.2);
                    // Bill looking right
                    gpu::draw_rect_flat(body_x + 14, body_y + 5, 5, 4, bill_col.0, bill_col.1, bill_col.2);
                    gpu::draw_rect_flat(body_x + 11, body_y + 4, 2, 2, 255, 255, 255);
                    // Feet gripping wall
                    gpu::draw_rect_flat(body_x + 1, body_y + 13, 4, 2, 60, 50, 45);
                    // Satchel
                    gpu::draw_rect_flat(body_x + 6, body_y + 4, 5, 4, satchel_col.0, satchel_col.1, satchel_col.2);
                } else {
                    // Right wall: tail pressed flat against right wall
                    gpu::draw_rect_flat(body_x + 12, body_y + 2, 4, 12, tail_col.0, tail_col.1, tail_col.2);
                    gpu::draw_rect_flat(body_x + 2, body_y + 3, 10, 10, body_col.0, body_col.1, body_col.2);
                    gpu::draw_rect_flat(body_x + 3, body_y + 5, 7, 6, belly_col.0, belly_col.1, belly_col.2);
                    // Bill looking left
                    gpu::draw_rect_flat(body_x - 3, body_y + 5, 5, 4, bill_col.0, bill_col.1, bill_col.2);
                    gpu::draw_rect_flat(body_x + 3, body_y + 4, 2, 2, 255, 255, 255);
                    // Feet gripping wall
                    gpu::draw_rect_flat(body_x + 11, body_y + 13, 4, 2, 60, 50, 45);
                    // Satchel
                    gpu::draw_rect_flat(body_x + 5, body_y + 4, 5, 4, satchel_col.0, satchel_col.1, satchel_col.2);
                }
            }
            PlayerState::HydroBreach => {
                // Rocket dolphin breach - torpedo upward
                let body_x = px;
                let body_y = py;
                gpu::draw_rect_flat(body_x + 3, body_y + 2, 10, 12, body_col.0, body_col.1, body_col.2);
                gpu::draw_rect_flat(body_x + 4, body_y + 4, 8, 7, belly_col.0, belly_col.1, belly_col.2);
                // Bill pointed skyward
                gpu::draw_rect_flat(body_x + 5, body_y - 4, 6, 6, bill_col.0, bill_col.1, bill_col.2);
                // Eyes
                gpu::draw_rect_flat(body_x + 4, body_y + 1, 2, 2, 255, 255, 255);
                gpu::draw_rect_flat(body_x + 10, body_y + 1, 2, 2, 255, 255, 255);
                // Tail trailing downward
                gpu::draw_rect_flat(body_x + 4, body_y + 14, 8, 5, tail_col.0, tail_col.1, tail_col.2);
                // Webbed flippers back
                gpu::draw_rect_flat(body_x, body_y + 7, 3, 5, 50, 45, 40);
                gpu::draw_rect_flat(body_x + 13, body_y + 7, 3, 5, 50, 45, 40);
                // Satchel
                gpu::draw_rect_flat(body_x + 5, body_y + 5, 6, 4, satchel_col.0, satchel_col.1, satchel_col.2);
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
            let cy_raw = (c.y - self.camera_y) as i16;
            if cx < -16 || cx > SCREEN_W || cy_raw < -16 || cy_raw > SCREEN_H {
                continue;
            }
            let bob = if (c.bob_timer / 10) % 2 == 0 { 1 } else { 0 };
            let cy = cy_raw + bob;

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
                CollectibleType::BuriedYabby => {
                    if platty.electro_timer > 0 {
                        // Fully illuminated golden treasure yabby!
                        gpu::draw_rect_flat(cx + 4, cy + 4, 8, 6, 255, 215, 40);
                        gpu::draw_rect_flat(cx + 2, cy + 2, 3, 3, 255, 180, 20);
                        gpu::draw_rect_flat(cx + 11, cy + 2, 3, 3, 255, 180, 20);
                        gpu::draw_rect_flat(cx + 6, cy + 10, 4, 4, 230, 150, 20);
                        // Radiant pulse ring
                        gpu::draw_line_mono(cx, cy + 8, cx + 8, cy, 120, 255, 255);
                        gpu::draw_line_mono(cx + 8, cy, cx + 16, cy + 8, 120, 255, 255);
                        gpu::draw_line_mono(cx + 16, cy + 8, cx + 8, cy + 16, 120, 255, 255);
                        gpu::draw_line_mono(cx + 8, cy + 16, cx, cy + 8, 120, 255, 255);
                    } else {
                        // Subtle buried silt mound
                        gpu::draw_rect_flat(cx + 5, cy + 10, 6, 4, 100, 75, 45);
                        gpu::draw_rect_flat(cx + 7, cy + 8, 2, 2, 80, 60, 35);
                    }
                }
            }
        }

        // Enemies
        for e in entities.enemies.iter() {
            if !e.active {
                continue;
            }
            let ex = (e.x.to_int() - self.camera_x) as i16;
            let ey = (e.y.to_int() - self.camera_y) as i16;
            if ex < -32 || ex > SCREEN_W + 32 || ey < -32 || ey > SCREEN_H + 32 {
                continue;
            }

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
            let py = (p.y - self.camera_y) as i16;
            if px >= 0 && px < SCREEN_W && py >= 0 && py < SCREEN_H {
                gpu::draw_rect_flat(px, py, p.size as u16, p.size as u16, p.color.0, p.color.1, p.color.2);
            }
        }
    }

    /// Render HUD overlay (Health hearts, Air meter, Score, Letter badge).
    pub fn draw_hud(&self, platty: &Platypus, act: Act) {
        // Sleek dark ribbon across top with metallic border
        gpu::draw_rect_flat(0, 0, 320, 20, 12, 16, 24);
        gpu::draw_rect_flat(0, 20, 320, 1, 45, 60, 85);

        // Hearts (Health) with 3D drop highlight
        for i in 0..platty.max_health {
            let hx = 8 + (i as i16 * 14);
            if i < platty.health {
                // Heart body
                gpu::draw_rect_flat(hx, 6, 10, 8, 235, 35, 55);
                gpu::draw_rect_flat(hx + 1, 4, 8, 3, 235, 35, 55);
                // Specular sheen
                gpu::draw_rect_flat(hx + 2, 5, 2, 2, 255, 180, 190);
            } else {
                // Empty heart outline
                gpu::draw_rect_flat(hx, 6, 10, 8, 55, 35, 42);
                gpu::draw_rect_flat(hx + 2, 8, 6, 4, 18, 20, 28);
            }
        }

        // Yabbies counter icon & text
        gpu::draw_rect_flat(68, 6, 8, 8, 45, 140, 245);
        gpu::draw_rect_flat(70, 8, 4, 4, 110, 210, 255);
        let mut yabby_buf = [b'x', b'0', b'0', 0];
        let y0 = ((platty.yabbies_collected / 10) % 10) as u8 + b'0';
        let y1 = (platty.yabbies_collected % 10) as u8 + b'0';
        yabby_buf[1] = y0;
        yabby_buf[2] = y1;
        if let Ok(s) = core::str::from_utf8(&yabby_buf[..3]) {
            self.font.draw_text(79, 6, s, (190, 230, 255));
        }

        // Air meter when underwater, otherwise Act title
        if platty.in_water {
            gpu::draw_rect_flat(120, 6, 64, 8, 18, 35, 55);
            gpu::draw_rect_flat(121, 7, 62, 6, 30, 60, 90);
            let air_w = ((platty.air as u32 * 60) / 100) as u16;
            let air_col = if platty.air < 30 { (255, 70, 60) } else { (70, 215, 255) };
            gpu::draw_rect_flat(122, 8, air_w, 4, air_col.0, air_col.1, air_col.2);
        } else {
            self.font.draw_text(118, 6, act.title(), (255, 225, 130));
        }

        // Score display (6-digit zero-padded)
        let mut score_buf = [b'0'; 6];
        let mut sc = platty.score;
        for i in (0..6).rev() {
            score_buf[i] = (sc % 10) as u8 + b'0';
            sc /= 10;
        }
        if let Ok(s) = core::str::from_utf8(&score_buf) {
            self.font.draw_text(215, 6, s, (245, 245, 245));
        }

        // Letter pouch badge
        gpu::draw_rect_flat(285, 4, 14, 11, 245, 240, 220);
        gpu::draw_rect_flat(289, 7, 6, 5, 210, 145, 45);
        gpu::draw_rect_flat(291, 9, 2, 2, 180, 50, 45); // Red wax seal
        self.font.draw_text(302, 6, "OK", (100, 255, 130));
    }

    /// Render Title Screen with Gouraud backdrop and character art.
    pub fn draw_title_screen(&self, frame: u8) {
        // Gouraud twilight ocean sky
        draw_gradient_rect(0, 0, 320, 120, (10, 25, 48), (28, 65, 95));
        draw_gradient_rect(0, 120, 320, 240, (28, 65, 95), (12, 32, 50));

        // Distant mountain silhouette
        gpu::draw_tri_gouraud([(0, 130), (70, 85), (150, 130)], [(16, 38, 55), (28, 58, 80), (16, 38, 55)]);
        gpu::draw_tri_gouraud([(120, 130), (220, 75), (320, 130)], [(16, 38, 55), (32, 65, 90), (16, 38, 55)]);

        // Title plaque shadow + golden border
        gpu::draw_rect_flat(38, 32, 244, 58, 8, 16, 24);
        gpu::draw_rect_flat(36, 30, 244, 58, 220, 185, 80);
        gpu::draw_rect_flat(38, 32, 240, 54, 18, 38, 58);

        // Title text with gold & sky blue styling
        let shimmer = if (frame / 12) % 2 == 0 { (255, 240, 120) } else { (255, 210, 70) };
        self.font.draw_text(105, 42, "P L A T T Y P U S", shimmer);
        self.font.draw_text(76, 63, "JOURNEY HOME TO THE NATIVE WILD", (170, 230, 255));

        // Center Platypus Hero with Satchel & Letter
        let cx = 145;
        let cy = 115;
        gpu::draw_rect_flat(cx, cy, 30, 14, 95, 55, 30);
        gpu::draw_rect_flat(cx + 25, cy + 3, 14, 8, 45, 42, 42); // Bill
        gpu::draw_rect_flat(cx + 27, cy + 5, 2, 2, 25, 25, 25);  // Nostril
        gpu::draw_rect_flat(cx + 20, cy + 2, 3, 3, 255, 255, 255); // Eye
        gpu::draw_rect_flat(cx + 21, cy + 3, 1, 1, 0, 0, 0);       // Pupil
        gpu::draw_rect_flat(cx - 16, cy + 2, 18, 10, 75, 42, 22); // Beaver tail
        gpu::draw_rect_flat(cx + 8, cy - 4, 11, 9, 175, 130, 65); // Leather Satchel
        gpu::draw_rect_flat(cx + 10, cy - 7, 7, 5, 255, 250, 235); // Important Letter!
        gpu::draw_rect_flat(cx + 12, cy - 5, 3, 2, 190, 45, 45);   // Red seal

        // Blinking start prompt
        if (frame / 20) % 2 == 0 {
            self.font.draw_text(92, 162, "PRESS START OR CROSS", (255, 255, 255));
        }

        self.font.draw_text(65, 205, "PSX HOMEBREW  *  POWERED BY PSOXIDE", (130, 170, 195));
    }

    /// Render Parents' Letter Intro cutscene with parchment shading.
    pub fn draw_letter_intro(&self, _frame: u8) {
        // Vignette gradient
        draw_gradient_rect(0, 0, 320, 240, (12, 16, 22), (22, 28, 38));

        // Parchment paper shadow + aged letter page
        gpu::draw_rect_flat(27, 23, 270, 194, 8, 10, 14);
        gpu::draw_rect_flat(24, 20, 270, 194, 215, 195, 160);
        gpu::draw_rect_flat(26, 22, 266, 190, 248, 238, 212);

        // Wax seal
        gpu::draw_rect_flat(142, 30, 36, 24, 180, 42, 40);
        gpu::draw_rect_flat(144, 32, 32, 20, 210, 60, 55);
        self.font.draw_text(152, 36, "P&M", (255, 235, 200));

        // Letter handwriting text
        self.font.draw_text(40, 65, "DEAREST PLATTY,", (50, 40, 30));
        self.font.draw_text(40, 85, "WE HAVE THE MOST WONDERFUL NEWS!", (50, 40, 30));
        self.font.draw_text(40, 105, "YOU ARE GOING TO BE A BIG BROTHER!", (180, 40, 40));
        self.font.draw_text(40, 125, "AN EGG HAS LAID IN THE COASTAL BURROW.", (50, 40, 30));
        self.font.draw_text(40, 145, "PLEASE HURRY HOME AS FAST AS YOU CAN!", (50, 40, 30));
        self.font.draw_text(170, 170, "- LOVE, MOM & DAD", (50, 40, 30));

        self.font.draw_text(75, 195, "PRESS CROSS TO SNEAK OUT!", (40, 110, 180));
    }

    /// Render Stage Clear screen with Gouraud plaque.
    pub fn draw_stage_clear(&self, act: Act, score: u32, yabbies: u16) {
        draw_gradient_rect(28, 38, 292, 202, (15, 30, 50), (25, 50, 75));
        gpu::draw_rect_flat(32, 42, 256, 156, 12, 22, 36);

        self.font.draw_text(90, 60, "STAGE COMPLETE!", (100, 255, 150));
        self.font.draw_text(60, 85, act.title(), (255, 220, 90));

        let mut yabbies_str = [b'Y', b'A', b'B', b'B', b'I', b'E', b'S', b':', b' ', b'0', b'0', 0];
        yabbies_str[9] = ((yabbies / 10) % 10) as u8 + b'0';
        yabbies_str[10] = (yabbies % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&yabbies_str[..11]) {
            self.font.draw_text(60, 115, s, (180, 220, 255));
        }

        let mut score_str = [b'S', b'C', b'O', b'R', b'E', b':', b' ', b'0', b'0', b'0', b'0', b'0', b'0', 0];
        let mut sc = score;
        for i in (7..13).rev() {
            score_str[i] = (sc % 10) as u8 + b'0';
            sc /= 10;
        }
        if let Ok(s) = core::str::from_utf8(&score_str[..13]) {
            self.font.draw_text(60, 135, s, (245, 245, 180));
        }

        self.font.draw_text(75, 170, "PRESS CROSS TO CONTINUE", (255, 255, 255));
    }

    /// Render Family Reunion Ending cutscene!
    pub fn draw_ending(&self, frame: u8) {
        // Coastal sunset / golden beach Gouraud gradients
        draw_gradient_rect(0, 0, 320, 120, (235, 95, 75), (255, 190, 110));
        draw_gradient_rect(0, 120, 320, 180, (255, 190, 110), (210, 180, 120));

        // Estuary river water
        draw_gradient_rect(0, 180, 320, 240, (40, 110, 160), (20, 60, 110));

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
