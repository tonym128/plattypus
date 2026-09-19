//! Tactical 3D Platypus player controller for Plattypus MGS.
//! Full 3D movement in X/Z with elevation Y, Belly-Crawl stealth mode,
//! Wall-Hug cover, Submersible swimming, and Electro-reception radar pulse.

use crate::audio::AudioManager;
use crate::entities::{CollectibleType, EntityManager, SentryState};
use crate::level::Level;
use psx_pad::{button, ButtonState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PlayerState {
    Standing,
    Sneaking,
    Running,
    BellyCrawl,
    Swimming,
    Submerged,
    WallHug,
    SpurStrike,
}

pub struct Platypus {
    pub x: i32,
    pub y: i32, // 0 = ground, negative = jumping, positive = submerged
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
            angle: 64, // Facing South (+Z) initially
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
        }
    }

    pub fn reset_position(&mut self, start_x: i32, start_z: i32) {
        self.x = start_x;
        self.y = 0;
        self.z = start_z;
        self.vx = 0;
        self.vy = 0;
        self.vz = 0;
        self.angle = 64;
        self.state = PlayerState::Standing;
        self.crawl_mode = false;
        self.electro_timer = 0;
        self.strike_timer = 0;
        self.invuln_timer = 60;
        self.screen_shake = 0;
    }

    pub fn update(
        &mut self,
        buttons: ButtonState,
        prev_buttons: ButtonState,
        level: &Level,
        entities: &mut EntityManager,
    ) {
        self.anim_frame = self.anim_frame.wrapping_add(1);

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

        let in_water = level.is_water_at(self.x, self.z);

        // Toggle Crawl / Stand (Cross button or Down while stationary)
        let just_cross = buttons.is_held(button::CROSS) && !prev_buttons.is_held(button::CROSS);
        let just_square = buttons.is_held(button::SQUARE) && !prev_buttons.is_held(button::SQUARE);
        let just_triangle = buttons.is_held(button::TRIANGLE) && !prev_buttons.is_held(button::TRIANGLE);

        if in_water {
            // In water: Cross toggles Submerged dive!
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
                // Air bubbles
                if self.anim_frame % 8 == 0 {
                    entities.spawn_particle(self.x, self.y, self.z, 0, -2, 0, 15, (180, 240, 255), 2);
                }
            } else {
                self.state = PlayerState::Swimming;
                self.y = 4;
                if self.air < 100 {
                    self.air = (self.air + 2).min(100); // Breathe at surface
                }
            }
        } else {
            // On land: Cross toggles Belly Crawl!
            if just_cross {
                self.crawl_mode = !self.crawl_mode;
                AudioManager::play_swoosh();
            }

            if self.crawl_mode {
                self.state = PlayerState::BellyCrawl;
                self.y = 0;
            } else {
                self.y = 0;
            }

            if self.air < 100 {
                self.air = 100;
            }
        }

        // Electro-Reception Radar Pulse (Triangle)
        if just_triangle && self.electro_timer == 0 {
            self.electro_timer = 50;
            AudioManager::play_electro();
            self.screen_shake = 3;

            // Spawn circular shockwave particles in 3D
            for i in 0..8 {
                let ang = (i as u16) * 32;
                let vx = (psx_gte_core::transform::cos_1_3_12(ang) as i32 * 3) >> 12;
                let vz = (psx_gte_core::transform::sin_1_3_12(ang) as i32 * 3) >> 12;
                entities.spawn_particle(self.x, self.y - 10, self.z, vx as i16, 0, vz as i16, 20, (100, 255, 240), 2);
            }

            // Stun nearby sentries & drones with electro-bill EMP!
            for s in entities.sentries.iter_mut() {
                if s.active {
                    let dx = (self.x - s.x).abs();
                    let dz = (self.z - s.z).abs();
                    if dx < 140 && dz < 140 {
                        s.stun_timer = 300; // 5 seconds stunned!
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
            // Reveal buried yabbies
            for c in entities.collectibles.iter_mut() {
                if c.active && c.kind == CollectibleType::BuriedYabby {
                    let dx = (self.x - c.x).abs();
                    let dz = (self.z - c.z).abs();
                    if dx < 120 && dz < 120 {
                        c.revealed = true;
                    }
                }
            }
        }

        // Tactical Venom Spur Strike (Square)
        if just_square && self.strike_timer == 0 {
            self.strike_timer = 16;
            self.state = PlayerState::SpurStrike;
            AudioManager::play_hit();

            // Check stealth takedown behind guard
            let mut spark_pos = None;
            for s in entities.sentries.iter_mut() {
                if s.active && s.stun_timer == 0 {
                    let dx = (self.x - s.x).abs();
                    let dz = (self.z - s.z).abs();
                    if dx < 36 && dz < 36 {
                        // Knock out sentry!
                        s.stun_timer = 500; // Long tactical knockout
                        s.state = SentryState::Stunned;
                        self.score += 200;
                        self.screen_shake = 5;
                        AudioManager::play_hit();
                        spark_pos = Some((s.x, s.y - 20, s.z));
                        break;
                    }
                }
            }
            if let Some((px, py, pz)) = spark_pos {
                entities.spawn_particle(px, py, pz, 0, -2, 0, 25, (255, 230, 80), 3);
            }
        }

        // Directional Movement input
        let btn_up = buttons.is_held(button::UP);
        let btn_down = buttons.is_held(button::DOWN);
        let btn_left = buttons.is_held(button::LEFT);
        let btn_right = buttons.is_held(button::RIGHT);

        let mut move_x = 0;
        let mut move_z = 0;

        if btn_up {
            move_z -= 1;
        }
        if btn_down {
            move_z += 1;
        }
        if btn_left {
            move_x -= 1;
        }
        if btn_right {
            move_x += 1;
        }

        let is_moving = move_x != 0 || move_z != 0;

        if is_moving {
            // Calculate movement angle
            if move_x > 0 && move_z == 0 {
                self.angle = 0; // East (+X)
            } else if move_x > 0 && move_z > 0 {
                self.angle = 32; // South-East
            } else if move_x == 0 && move_z > 0 {
                self.angle = 64; // South (+Z)
            } else if move_x < 0 && move_z > 0 {
                self.angle = 96; // South-West
            } else if move_x < 0 && move_z == 0 {
                self.angle = 128; // West (-X)
            } else if move_x < 0 && move_z < 0 {
                self.angle = 160; // North-West
            } else if move_x == 0 && move_z < 0 {
                self.angle = 192; // North (-Z)
            } else if move_x > 0 && move_z < 0 {
                self.angle = 224; // North-East
            }

            let speed = if self.crawl_mode {
                2 // Silent slow belly-crawl
            } else if in_water {
                if self.state == PlayerState::Submerged { 3 } else { 2 }
            } else {
                3 // Normal stealth jog
            };

            self.vx = move_x * speed;
            self.vz = move_z * speed;

            if !self.crawl_mode && !in_water {
                self.state = PlayerState::Sneaking;
                if self.step_audio_timer == 0 {
                    AudioManager::play_waddle();
                    self.step_audio_timer = 20;
                }
            }
        } else {
            self.vx = 0;
            self.vz = 0;
            if !self.crawl_mode && !in_water && self.strike_timer == 0 {
                self.state = PlayerState::Standing;
            }
        }

        // Apply 3D movement and collision check against level geometry
        let next_x = self.x + self.vx;
        let next_z = self.z + self.vz;

        let col_radius = if self.crawl_mode { 8 } else { 12 };

        // X collision
        if !level.is_solid_at(next_x + col_radius, self.z, self.crawl_mode)
            && !level.is_solid_at(next_x - col_radius, self.z, self.crawl_mode)
        {
            self.x = next_x;
        } else {
            self.vx = 0;
        }

        // Z collision
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
            if dx < 20 && dz < 20 {
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
                        self.electro_timer = 0; // Instantly ready
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
                }
            }
        }

        // Enemy collision damage
        for s in entities.sentries.iter() {
            if s.active && s.stun_timer == 0 {
                let dx = (self.x - s.x).abs();
                let dz = (self.z - s.z).abs();
                if dx < 18 && dz < 18 {
                    self.take_damage(1);
                    self.vx = if self.x < s.x { -4 } else { 4 };
                    self.vz = if self.z < s.z { -4 } else { 4 };
                }
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
        AudioManager::play_hit();
    }
}
