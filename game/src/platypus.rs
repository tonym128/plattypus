//! Tactical 3D Platypus player controller for Plattypus MGS.
//! Full 3D movement in X/Z with elevation Y, jumping with gravity,
//! belly-crawl stealth mode, water diving, and electro-reception radar pulse.

use crate::audio::AudioManager;
use crate::entities::{CollectibleType, EntityManager, MechState, RiverObstacleType, SentryState};
use crate::level::{Act, CellType, Level, TILE_SZ};
use psx_gte_core::transform::{cos_1_3_12, sin_1_3_12};
use psx_pad::{button, ButtonState, Deadzone, PadState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PlayerState {
    Standing,
    Sneaking,
    Running,
    Jumping,
    BellyCrawl,
    Swimming,
    Submerged,
    SpurStrike,
}

pub struct Platypus {
    pub x: i32,
    pub y: i32, // 0 = ground, negative = in air, positive = submerged
    pub z: i32,
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    pub angle: u16, // 0..256
    pub state: PlayerState,

    pub health: u8,
    pub max_health: u8,
    pub air: u8,
    pub yabbies_collected: u16,
    pub score: u32,

    pub crawl_mode: bool,
    pub electro_timer: u8,
    pub strike_timer: u8,
    pub invuln_timer: u8,
    pub screen_shake: u8,
    pub anim_frame: u8,
    pub step_audio_timer: u8,
    pub on_ground: bool,

    // DualShock Vibration Timers
    pub rumble_small_timer: u8,
    pub rumble_large_timer: u8,
    pub rumble_large_intensity: u8,
}

impl Platypus {
    pub fn new(start_x: i32, start_z: i32) -> Self {
        Self {
            x: start_x,
            y: 0,
            z: start_z,
            vx: 0,
            vy: 0,
            vz: 0,
            angle: 0, // Facing South (+Z)
            state: PlayerState::Standing,
            health: 3,
            max_health: 3,
            air: 100,
            yabbies_collected: 0,
            score: 0,
            crawl_mode: false,
            electro_timer: 0,
            strike_timer: 0,
            invuln_timer: 0,
            screen_shake: 0,
            anim_frame: 0,
            step_audio_timer: 0,
            on_ground: true,
            rumble_small_timer: 0,
            rumble_large_timer: 0,
            rumble_large_intensity: 0,
        }
    }

    pub fn reset_position(&mut self, start_x: i32, start_z: i32) {
        self.x = start_x;
        self.y = 0;
        self.z = start_z;
        self.vx = 0;
        self.vy = 0;
        self.vz = 0;
        self.angle = 0;
        self.state = PlayerState::Standing;
        self.crawl_mode = false;
        self.electro_timer = 0;
        self.strike_timer = 0;
        self.invuln_timer = 60;
        self.screen_shake = 0;
        self.on_ground = true;
        self.rumble_small_timer = 0;
        self.rumble_large_timer = 0;
        self.rumble_large_intensity = 0;
        self.health = self.max_health;
        self.air = 100;
        self.yabbies_collected = 0;
        self.score = 0;
    }

    pub fn trigger_rumble_small(&mut self, duration: u8) {
        self.rumble_small_timer = self.rumble_small_timer.max(duration);
    }

    pub fn trigger_rumble_large(&mut self, duration: u8, intensity: u8) {
        self.rumble_large_timer = self.rumble_large_timer.max(duration);
        self.rumble_large_intensity = self.rumble_large_intensity.max(intensity);
    }

    pub fn get_rumble_state(&self) -> (bool, u8) {
        let small = self.rumble_small_timer > 0;
        let large = if self.rumble_large_timer > 0 {
            self.rumble_large_intensity
        } else {
            0
        };
        (small, large)
    }

    pub fn update(
        &mut self,
        pad: &PadState,
        prev_buttons: ButtonState,
        level: &Level,
        entities: &mut EntityManager,
    ) {
        let buttons = pad.buttons;
        self.anim_frame = self.anim_frame.wrapping_add(1);

        if self.rumble_small_timer > 0 {
            self.rumble_small_timer -= 1;
        }
        if self.rumble_large_timer > 0 {
            self.rumble_large_timer -= 1;
            if self.rumble_large_timer == 0 {
                self.rumble_large_intensity = 0;
            }
        }

        if self.invuln_timer > 0 {
            self.invuln_timer -= 1;
        }
        if self.electro_timer > 0 {
            self.electro_timer -= 1;
        }
        if self.strike_timer > 0 {
            self.strike_timer -= 1;
        }
        if self.step_audio_timer > 0 {
            self.step_audio_timer -= 1;
        }

        let in_water = level.is_water_at(self.x, self.z) && level.act != Act::Act4Ocean;

        let just_cross = buttons.is_held(button::CROSS) && !prev_buttons.is_held(button::CROSS);
        let just_circle = buttons.is_held(button::CIRCLE) && !prev_buttons.is_held(button::CIRCLE);
        let just_square = buttons.is_held(button::SQUARE) && !prev_buttons.is_held(button::SQUARE);
        let just_triangle = buttons.is_held(button::TRIANGLE) && !prev_buttons.is_held(button::TRIANGLE);

        // Water submersion vs Land Jump/Crawl
        if in_water {
            if buttons.is_held(button::CROSS) {
                self.state = PlayerState::Submerged;
                self.y = 18; // Submerged depth
                if self.air > 0 {
                    if self.anim_frame % 2 == 0 {
                        self.air -= 1;
                    }
                } else {
                    self.take_damage(1);
                }
                if self.anim_frame % 8 == 0 {
                    entities.spawn_particle(self.x, self.y, self.z, 0, -2, 0, 15, (180, 240, 255), 2);
                }
            } else {
                self.state = PlayerState::Swimming;
                self.y = 4;
                if self.air < 100 {
                    self.air = (self.air + 2).min(100);
                }
            }
        } else {
            // Jump on CROSS
            if just_cross && self.on_ground {
                self.vy = -11; // Jump impulse
                self.on_ground = false;
                self.state = PlayerState::Jumping;
                AudioManager::play_jump();
            }

            // Crawl / Crouch on CIRCLE
            if just_circle {
                self.crawl_mode = !self.crawl_mode;
                AudioManager::play_swoosh();
            }

            // Gravity & Vertical physics
            let mut target_ground_y = 0i32;

            // In Act 4 (Beach platformer), check 3D elevated platforms
            if level.act == Act::Act4Ocean {
                for p in entities.beach_platforms.iter() {
                    if p.active {
                        let in_x = self.x >= p.x - 12 && self.x <= p.x + p.w + 12;
                        let in_z = self.z >= p.z - 12 && self.z <= p.z + p.d + 12;
                        if in_x && in_z {
                            if p.is_parasol {
                                // Bouncing umbrella launches Platty up!
                                if self.vy > 0 && (self.y - p.y).abs() < 24 {
                                    self.vy = -18; // Super Jump!
                                    self.on_ground = false;
                                    self.state = PlayerState::Jumping;
                                    self.screen_shake = 4;
                                    self.trigger_rumble_large(8, 160);
                                    AudioManager::play_jump();
                                }
                            } else if self.y <= p.y + 4 {
                                target_ground_y = target_ground_y.min(p.y);
                            }
                        }
                    }
                }
            }

            if !self.on_ground {
                self.vy += 1; // Gravity
                self.y += self.vy;

                if self.y >= target_ground_y {
                    self.y = target_ground_y;
                    self.vy = 0;
                    self.on_ground = true;
                    if self.crawl_mode {
                        self.state = PlayerState::BellyCrawl;
                    } else {
                        self.state = PlayerState::Standing;
                    }
                }
            } else {
                self.y = target_ground_y;
                if self.crawl_mode {
                    self.state = PlayerState::BellyCrawl;
                }
            }

            if self.air < 100 {
                self.air = 100;
            }
        }

        // Electro-Reception Radar Pulse (Triangle)
        if just_triangle && self.electro_timer == 0 {
            self.electro_timer = 50;
            self.trigger_rumble_small(12);
            AudioManager::play_electro();
            self.screen_shake = 3;

            for i in 0..8 {
                let ang = (i as u16) * 32;
                let vx = (cos_1_3_12(ang) as i32 * 3) >> 12;
                let vz = (sin_1_3_12(ang) as i32 * 3) >> 12;
                entities.spawn_particle(self.x, self.y - 10, self.z, vx as i16, 0, vz as i16, 20, (100, 255, 240), 2);
            }

            // Stun nearby sentries & drones
            for s in entities.sentries.iter_mut() {
                if s.active {
                    let dx = (self.x - s.x).abs();
                    let dz = (self.z - s.z).abs();
                    if dx < 140 && dz < 140 {
                        s.stun_timer = 300;
                        s.state = SentryState::Stunned;
                    }
                }
            }
            for d in entities.drones.iter_mut() {
                if d.active {
                    let dx = (self.x - d.x).abs();
                    let dz = (self.z - d.z).abs();
                    if dx < 160 && dz < 160 {
                        d.stun_timer = 300;
                    }
                }
            }
            for c in entities.collectibles.iter_mut() {
                if c.active && (c.kind == CollectibleType::BuriedYabby || c.kind == CollectibleType::StarYabby) {
                    let dx = (self.x - c.x).abs();
                    let dz = (self.z - c.z).abs();
                    if dx < 140 && dz < 140 {
                        c.revealed = true;
                    }
                }
            }
        }

        // Tactical Venom Spur Strike (Square)
        if just_square && self.strike_timer == 0 {
            self.strike_timer = 16;
            self.state = PlayerState::SpurStrike;
            AudioManager::play_spur();

            let mut spark_pos = None;
            for s in entities.sentries.iter_mut() {
                if s.active && s.stun_timer == 0 {
                    let dx = (self.x - s.x).abs();
                    let dz = (self.z - s.z).abs();
                    if dx < 40 && dz < 40 {
                        s.stun_timer = 500;
                        s.state = SentryState::Stunned;
                        self.score += 200;
                        self.screen_shake = 5;
                        self.trigger_rumble_large(8, 220);
                        self.trigger_rumble_small(6);
                        AudioManager::play_hit();
                        spark_pos = Some((s.x, s.y - 20, s.z));
                        break;
                    }
                }
            }

            // Boss Act 1: Attack Power Conduits & Mech Rear Core
            if level.act == Act::Act1Boss {
                for c in entities.power_conduits.iter_mut() {
                    if c.active && !c.destroyed {
                        let dx = (self.x - c.x).abs();
                        let dz = (self.z - c.z).abs();
                        if dx < 48 && dz < 48 {
                            if c.health > 1 {
                                c.health -= 1;
                                c.spark_timer = 20;
                                self.score += 150;
                                AudioManager::play_metal();
                                self.trigger_rumble_small(8);
                                spark_pos = Some((c.x, -20, c.z));
                            } else {
                                c.health = 0;
                                c.destroyed = true;
                                self.score += 500;
                                self.screen_shake = 8;
                                self.trigger_rumble_large(16, 255);
                                AudioManager::play_hit();
                                spark_pos = Some((c.x, -20, c.z));
                            }
                            break;
                        }
                    }
                }

                // Attack Mech Rear Coolant Core
                let mech = &mut entities.boss_mech;
                if mech.active && !mech.shield_active && !mech.is_defeated() {
                    let dx = (self.x - mech.x).abs();
                    // Behind the mech (closer to north than the mech body or within close proximity from behind)
                    let is_behind = self.z <= mech.z + 16 && self.z >= mech.z - 44;
                    if dx < 44 && is_behind && mech.hit_timer == 0 {
                        mech.hit_timer = 30;
                        if mech.health > 1 {
                            mech.health -= 1;
                            self.score += 1000;
                            self.screen_shake = 10;
                            self.trigger_rumble_large(20, 255);
                            self.trigger_rumble_small(14);
                            AudioManager::play_hit();
                            spark_pos = Some((mech.x, -26, mech.z - 16));
                        } else {
                            mech.health = 0;
                            mech.state = MechState::Defeated(120);
                            self.score += 5000;
                            self.screen_shake = 16;
                            self.trigger_rumble_large(32, 255);
                            AudioManager::play_fanfare();
                            spark_pos = Some((mech.x, -26, mech.z - 16));
                        }
                    }
                }
            }

            if let Some((px, py, pz)) = spark_pos {
                entities.spawn_particle(px, py, pz, 0, -2, 0, 25, (255, 230, 80), 3);
            }
        }

        // DualShock Analog Stick vs Digital D-pad directional movement
        let (lx, ly) = pad.sticks.left_centered();
        let dz = Deadzone::new(18);
        let analog_stick = if pad.mode.has_sticks() {
            dz.scaled(lx, ly)
        } else {
            None
        };

        let mut is_analog_moving = false;

        if let Some((sx, sy)) = analog_stick {
            let sx_i32 = sx as i32;
            let sy_i32 = sy as i32;
            let mag = psx_math::int32::isqrt_i32(sx_i32 * sx_i32 + sy_i32 * sy_i32);

            if mag > 0 {
                is_analog_moving = true;

                // True 360-degree angle from analog stick via atan2_q12
                // atan2_q12(sx, sy) returns 0..4096. Shifting >> 4 gives 0..256 matching GTE rotation.
                let raw_angle = psx_math::atan2_q12(sx_i32, sy_i32);
                self.angle = (raw_angle >> 4) as u16;

                // Analog speed curve:
                // mag < 65: Slow tilt = stealth stalk/sneak mode (silent movement, no sentry alert)
                // mag >= 65: Hard push = full sprint
                let max_speed = if self.crawl_mode {
                    2
                } else if in_water {
                    if self.state == PlayerState::Submerged { 2 } else { 3 }
                } else if !self.on_ground {
                    4
                } else if mag < 65 {
                    2 // Stalking / sneak speed
                } else {
                    4 // Full run speed
                };

                // Scale velocity smoothly with analog stick deflection
                let speed = (max_speed * mag) / 127;
                let speed = speed.max(1);

                self.vx = (sx_i32 * speed) / 127;
                // Invert analog Y: stick UP (negative sy) = forward (+Z), stick DOWN = backward (-Z)
                self.vz = (-sy_i32 * speed) / 127;

                if level.act == Act::Act2Bushland {
                    self.vz += 2; // Rushing downriver!
                }

                if self.on_ground && !self.crawl_mode && self.strike_timer == 0 {
                    if mag < 65 {
                        self.state = PlayerState::Sneaking;
                        // Stealth stalk: NO footstep sound! Silent paws.
                    } else {
                        self.state = PlayerState::Running;
                        if self.step_audio_timer == 0 {
                            self.step_audio_timer = 14;
                            let surface = match level.act {
                                Act::Act1Sanctuary | Act::Act1Boss => crate::audio::SurfaceType::Concrete,
                                Act::Act2Bushland => crate::audio::SurfaceType::Grass,
                                Act::Act3City => {
                                    self.trigger_rumble_small(3);
                                    crate::audio::SurfaceType::Metal
                                }
                                Act::Act4Ocean => crate::audio::SurfaceType::Grass,
                            };
                            AudioManager::play_footstep(surface);
                        }
                    }
                }
            }
        }

        if !is_analog_moving {
            // Digital D-pad fallback
            let btn_up = buttons.is_held(button::UP);
            let btn_down = buttons.is_held(button::DOWN);
            let btn_left = buttons.is_held(button::LEFT);
            let btn_right = buttons.is_held(button::RIGHT);

            let mut move_x = 0;
            let mut move_z = 0;

            // UP = forward (South / +Z), DOWN = backward (North / -Z)
            if btn_up {
                move_z += 1;
            }
            if btn_down {
                move_z -= 1;
            }
            if btn_left {
                move_x -= 1;
            }
            if btn_right {
                move_x += 1;
            }

            // Automatic river current push in Act 2
            if level.act == Act::Act2Bushland {
                move_z += 2; // Rushing downriver!
            }

            let speed = if self.crawl_mode {
                2
            } else if in_water {
                if self.state == PlayerState::Submerged { 2 } else { 3 }
            } else if !self.on_ground {
                4 // Air mobility
            } else {
                4 // Normal run
            };

            if move_x != 0 || move_z != 0 {
                self.vx = move_x * speed;
                self.vz = move_z * speed;

                // Facing angle: 0=South (+Z), 64=East (+X), 128=North (-Z), 192=West (-X)
                if move_x > 0 && move_z == 0 {
                    self.angle = 64;
                } else if move_x < 0 && move_z == 0 {
                    self.angle = 192;
                } else if move_z > 0 && move_x == 0 {
                    self.angle = 0;
                } else if move_z < 0 && move_x == 0 {
                    self.angle = 128;
                } else if move_x > 0 && move_z > 0 {
                    self.angle = 32;
                } else if move_x > 0 && move_z < 0 {
                    self.angle = 96;
                } else if move_x < 0 && move_z > 0 {
                    self.angle = 224;
                } else if move_x < 0 && move_z < 0 {
                    self.angle = 160;
                }

                if self.on_ground && !self.crawl_mode && self.strike_timer == 0 {
                    self.state = PlayerState::Running;
                    if self.step_audio_timer == 0 {
                        self.step_audio_timer = 14;
                        let surface = match level.act {
                            Act::Act1Sanctuary | Act::Act1Boss => crate::audio::SurfaceType::Concrete,
                            Act::Act2Bushland => crate::audio::SurfaceType::Grass,
                            Act::Act3City => {
                                self.trigger_rumble_small(3);
                                crate::audio::SurfaceType::Metal
                            }
                            Act::Act4Ocean => crate::audio::SurfaceType::Grass,
                        };
                        AudioManager::play_footstep(surface);
                    }
                }
            } else {
                self.vx = 0;
                if level.act != Act::Act2Bushland {
                    self.vz = 0;
                }
                if self.on_ground && !self.crawl_mode && self.strike_timer == 0 && !in_water {
                    self.state = PlayerState::Standing;
                }
            }
        }

        // Apply movement & collision
        let col_radius = 12;
        let next_x = self.x + self.vx;
        let next_z = self.z + self.vz;

        if !level.is_solid_at(next_x + col_radius, self.z, self.crawl_mode)
            && !level.is_solid_at(next_x - col_radius, self.z, self.crawl_mode)
        {
            self.x = next_x;
        } else {
            self.vx = 0;
        }

        if !level.is_solid_at(self.x, next_z + col_radius, self.crawl_mode)
            && !level.is_solid_at(self.x, next_z - col_radius, self.crawl_mode)
        {
            self.z = next_z;
        } else {
            self.vz = 0;
        }

        // Collectibles pickup
        for c in entities.collectibles.iter_mut() {
            if !c.active || !c.revealed {
                continue;
            }
            let dx = (self.x - c.x).abs();
            let dz = (self.z - c.z).abs();
            if dx < 22 && dz < 22 {
                c.active = false;
                match c.kind {
                    CollectibleType::YabbyRation => {
                        self.yabbies_collected += 1;
                        self.score += 100;
                        if self.health < self.max_health {
                            self.health += 1;
                        }
                        AudioManager::play_yabby();
                    }
                    CollectibleType::ChaffBattery => {
                        self.score += 150;
                        self.electro_timer = 0;
                        AudioManager::play_fanfare();
                    }
                    CollectibleType::LetterPage => {
                        self.score += 250;
                        AudioManager::play_fanfare();
                    }
                    CollectibleType::BuriedYabby => {
                        self.yabbies_collected += 1;
                        self.score += 300;
                        self.health = self.max_health;
                        AudioManager::play_fanfare();
                    }
                    CollectibleType::StarYabby => {
                        self.yabbies_collected += 2;
                        self.score += 500;
                        AudioManager::play_fanfare();
                    }
                }
            }
        }

        // Stage 1 Enemy Collision & Attack Check
        if level.act == Act::Act1Sanctuary {
            for s in entities.sentries.iter_mut() {
                if s.active && s.stun_timer == 0 {
                    let dx = (self.x - s.x).abs();
                    let dz = (self.z - s.z).abs();
                    if dx < 28 && dz < 28 && s.attack_cooldown == 0 {
                        s.attack_cooldown = 45;
                        self.take_damage(1);
                        self.vx = if self.x < s.x { -6 } else { 6 };
                        self.vz = if self.z < s.z { -6 } else { 6 };
                    }
                }
            }
        }

        // Stage 1 Climax Boss Collision & Shockwave Check
        if level.act == Act::Act1Boss {
            let mech = &entities.boss_mech;
            if mech.active && !mech.is_defeated() {
                // Shockwave Stomp check
                if mech.shockwave_active {
                    let dx = self.x - mech.x;
                    let dz = self.z - mech.z;
                    let dist = psx_math::int32::isqrt_i32(dx * dx + dz * dz);
                    if (dist - mech.shockwave_radius).abs() < 24 {
                        let gx = (self.x / TILE_SZ).clamp(0, 23) as usize;
                        let gz = (self.z / TILE_SZ).clamp(0, 23) as usize;
                        let in_trench = level.get_cell(gx, gz) == CellType::AirDuct && self.crawl_mode;
                        if self.on_ground && !in_trench {
                            self.take_damage(1);
                            self.vz = 8;
                            self.screen_shake = 6;
                            self.trigger_rumble_large(16, 255);
                        }
                    }
                }

                // Physical Mech body collision
                let dx = (self.x - mech.x).abs();
                let dz = (self.z - mech.z).abs();
                if dx < 36 && dz < 36 && mech.y >= -10 {
                    self.take_damage(1);
                    self.vz = 8;
                }
            }
        }

        // Stage 2 River Obstacles Collision Check
        if level.act == Act::Act2Bushland {
            for obs in entities.river_obstacles.iter() {
                if !obs.active {
                    continue;
                }
                let dx = (self.x - obs.x).abs();
                let dz = (self.z - obs.z).abs();
                if dx < 24 && dz < 24 {
                    match obs.kind {
                        RiverObstacleType::LowBranch => {
                            // Can only pass if ducking / crawling
                            if !self.crawl_mode {
                                self.take_damage(1);
                                self.vz = -4;
                            }
                        }
                        RiverObstacleType::TreeLog | RiverObstacleType::TigerSnake | RiverObstacleType::GiantSpider => {
                            // Can jump over!
                            if self.on_ground {
                                self.take_damage(1);
                                self.vz = -4;
                            }
                        }
                        RiverObstacleType::RiverTuber | RiverObstacleType::PaddleBoarder | RiverObstacleType::Swimmer => {
                            self.take_damage(1);
                            self.vz = -4;
                        }
                        RiverObstacleType::Koala => {} // Harmless cute koala
                    }
                }
            }
        }

        // Stage 3 City Traffic Collision Check
        if level.act == Act::Act3City {
            for v in entities.vehicles.iter() {
                if !v.active {
                    continue;
                }
                let dx = (self.x - v.x).abs();
                let dz = (self.z - v.z).abs();
                if dx < (v.length / 2 + 10) && dz < 18 {
                    // Car hits Platty! Horn honks!
                    self.take_damage(1);
                    self.vz = 8; // Knock back toward south sidewalk
                    AudioManager::play_metal();
                }
            }
        }

        // Stage 4 Beach Platformer Crabs Collision Check
        if level.act == Act::Act4Ocean {
            let mut crab_stomped_pos = None;
            for crab in entities.beach_crabs.iter_mut() {
                if !crab.active {
                    continue;
                }
                let dx = (self.x - crab.x).abs();
                let dz = (self.z - crab.z).abs();
                if dx < 22 && dz < 22 {
                    if !self.on_ground && self.vy > 0 {
                        // Stomp on crab Mario style!
                        crab.active = false;
                        self.vy = -10; // Bounce up!
                        self.score += 150;
                        self.trigger_rumble_small(6);
                        AudioManager::play_hit();
                        crab_stomped_pos = Some((crab.x, crab.y, crab.z));
                        break;
                    } else {
                        // Pinched by crab!
                        self.take_damage(1);
                        self.vx = if self.x < crab.x { -5 } else { 5 };
                    }
                }
            }
            if let Some((cx, cy, cz)) = crab_stomped_pos {
                entities.spawn_particle(cx, cy, cz, 0, -2, 0, 20, (230, 80, 40), 3);
            }
        }
    }

    pub fn take_damage(&mut self, amount: u8) {
        if self.invuln_timer > 0 {
            return;
        }
        if self.health > amount {
            self.health -= amount;
        } else {
            self.health = 0;
        }
        self.invuln_timer = 60;
        self.screen_shake = 8;
        self.trigger_rumble_large(14, 255);
        self.trigger_rumble_small(10);
        AudioManager::play_hit();
    }
}
