//! Platty the Platypus player controller, states, physics, and actions.

use crate::audio::AudioManager;
use crate::entities::EntityManager;
use crate::fixed::Fixed;
use crate::level::{Level, TileType, TILE_SIZE};
use psx_pad::{button, ButtonState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PlayerState {
    Standing,
    Running,
    Jumping,
    Fluttering,
    BellySlide,
    Swimming,
    TailWhip,
    SpurStomp,
    WallSlide,
    HydroBreach,
}

pub struct Platypus {
    pub x: Fixed,
    pub y: Fixed,
    pub vx: Fixed,
    pub vy: Fixed,
    pub facing_right: bool,
    pub state: PlayerState,
    pub on_ground: bool,
    pub in_water: bool,
    pub wall_slide_side: i8,

    pub health: u8,
    pub max_health: u8,
    pub air: u8,
    pub yabbies_collected: u16,
    pub score: u32,

    pub slide_timer: u8,
    pub whip_timer: u8,
    pub electro_timer: u8,
    pub invuln_timer: u8,
    pub anim_frame: u8,
    pub waddle_audio_cooldown: u8,
}

impl Platypus {
    pub fn new(start_x: Fixed, start_y: Fixed) -> Self {
        Self {
            x: start_x,
            y: start_y,
            vx: Fixed::ZERO,
            vy: Fixed::ZERO,
            facing_right: true,
            state: PlayerState::Standing,
            on_ground: false,
            in_water: false,
            wall_slide_side: 0,
            health: 3,
            max_health: 3,
            air: 100,
            yabbies_collected: 0,
            score: 0,
            slide_timer: 0,
            whip_timer: 0,
            electro_timer: 0,
            invuln_timer: 0,
            anim_frame: 0,
            waddle_audio_cooldown: 0,
        }
    }

    pub fn reset_position(&mut self, start_x: Fixed, start_y: Fixed) {
        self.x = start_x;
        self.y = start_y;
        self.vx = Fixed::ZERO;
        self.vy = Fixed::ZERO;
        self.state = PlayerState::Standing;
        self.wall_slide_side = 0;
        self.slide_timer = 0;
        self.whip_timer = 0;
        self.electro_timer = 0;
        self.invuln_timer = 60;
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

        if self.waddle_audio_cooldown > 0 {
            self.waddle_audio_cooldown -= 1;
        }

        let px = self.x.to_int();
        let py = self.y.to_int();

        let was_in_water = self.in_water;
        // Check if currently immersed in water
        self.in_water = level.is_water_at(px + 8, py + 8) || level.is_water_at(px + 8, py + 14);

        if self.in_water {
            self.update_swimming(buttons, prev_buttons, level, entities);
        } else {
            // Hydro-Breach Launch when rocketing out of water!
            if was_in_water && (self.vy < -Fixed::from_int(1) || buttons.is_held(button::CROSS)) {
                self.vy = -Fixed::from_fraction(11, 2);
                self.state = PlayerState::HydroBreach;
                AudioManager::play_swoosh();
                for sp in -2..=2 {
                    entities.spawn_particle(px + 8 + sp * 4, py + 12, (sp * 2) as i16, -3, 20, (160, 220, 255), 2);
                }
            }
            self.update_land(buttons, prev_buttons, level, entities);
        }

        // Electro-Reception trigger (Triangle)
        let just_triangle = buttons.is_held(button::TRIANGLE) && !prev_buttons.is_held(button::TRIANGLE);
        if just_triangle && self.electro_timer == 0 {
            self.electro_timer = 45; // 45 frames of active electro-sense wave
            AudioManager::play_electro();
            // Spawn electric sparks around bill
            let bill_x = if self.facing_right { px + 16 } else { px - 4 };
            entities.spawn_particle(bill_x, py + 4, 1, -1, 20, (100, 255, 255), 2);
            entities.spawn_particle(bill_x, py + 4, -1, 1, 20, (255, 255, 100), 2);
        }

        if self.electro_timer > 0 {
            self.electro_timer -= 1;
        }

        // Check collectibles collision
        self.check_collectibles(entities);

        // Check enemy collision & tail whip / spur stomp
        self.check_enemies(entities);

        // Check hazards
        if level.is_hazard_at(px + 8, py + 12) {
            self.take_damage(1);
        }
    }

    fn update_land(
        &mut self,
        buttons: ButtonState,
        prev_buttons: ButtonState,
        level: &Level,
        entities: &mut EntityManager,
    ) {
        // Refill air on land
        if self.air < 100 {
            self.air = 100;
        }

        let px = self.x.to_int();
        let py = self.y.to_int();

        let btn_left = buttons.is_held(button::LEFT);
        let btn_right = buttons.is_held(button::RIGHT);
        let btn_down = buttons.is_held(button::DOWN);
        let btn_cross = buttons.is_held(button::CROSS);
        let just_cross = btn_cross && !prev_buttons.is_held(button::CROSS);
        let just_square = buttons.is_held(button::SQUARE) && !prev_buttons.is_held(button::SQUARE);

        // Tail whip attack (Square) on ground or normal jump
        if just_square && self.whip_timer == 0 && !btn_down {
            self.whip_timer = 12;
            self.state = PlayerState::TailWhip;
            AudioManager::play_swoosh();
        }

        if self.whip_timer > 0 {
            self.whip_timer -= 1;
        }

        // Venom-Spur Stomp (Mid-air Down + Square)
        if !self.on_ground && btn_down && just_square && self.state != PlayerState::SpurStomp {
            self.state = PlayerState::SpurStomp;
            self.vy = Fixed::from_int(7); // High-velocity drop
            self.vx = Fixed::ZERO;
            AudioManager::play_swoosh();
            entities.spawn_particle(px + 8, py + 4, 0, -2, 18, (210, 90, 255), 2);
        }

        // Slide momentum jump: launch forward from belly slide!
        if self.state == PlayerState::BellySlide && just_cross {
            self.vy = -Fixed::from_fraction(7, 2);
            self.state = PlayerState::Jumping;
            self.slide_timer = 0;
            self.on_ground = false;
            AudioManager::play_jump();
            entities.spawn_particle(px + 8, py + 14, 0, 1, 12, (230, 200, 140), 2);
        } else if self.on_ground && btn_down && (just_cross || self.vx.abs() > Fixed::from_int(1)) && self.slide_timer == 0 {
            // Initiate Belly slide (Down + Cross or Down at speed)
            self.slide_timer = 30;
            self.state = PlayerState::BellySlide;
            AudioManager::play_swoosh();
            let boost = if self.facing_right { Fixed::from_int(3) } else { -Fixed::from_int(3) };
            self.vx = boost;
        }

        if self.slide_timer > 0 {
            self.slide_timer -= 1;
            self.state = PlayerState::BellySlide;

            // Spawn mud/dust particles while sliding
            if self.anim_frame % 3 == 0 {
                let p_vx = if self.facing_right { -1 } else { 1 };
                entities.spawn_particle(px + 8, py + 14, p_vx, -1, 10, (180, 140, 90), 2);
            }

            // Friction while sliding is low
            self.vx = self.vx * Fixed::from_fraction(95, 100);
        } else if self.state == PlayerState::SpurStomp {
            // Keep dropping fast until collision handles landing
            self.vy = Fixed::from_int(7);
            if self.anim_frame % 2 == 0 {
                entities.spawn_particle(px + 8, py + 4, 0, -1, 8, (190, 80, 240), 2);
            }
        } else {
            // Check Tail Wall-Slide in mid-air
            let mut is_wall_sliding = false;
            if !self.on_ground && self.vy > Fixed::ZERO {
                let left_wall = level.is_solid_at(px, py + 4) || level.is_solid_at(px, py + 12);
                let right_wall = level.is_solid_at(px + 16, py + 4) || level.is_solid_at(px + 16, py + 12);

                if left_wall && btn_left {
                    is_wall_sliding = true;
                    self.state = PlayerState::WallSlide;
                    self.wall_slide_side = -1;
                    self.facing_right = true; // Tail flat against left wall, looking right
                    self.vy = Fixed::from_fraction(1, 2); // Slow descent
                    if self.anim_frame % 4 == 0 {
                        entities.spawn_particle(px + 2, py + 10, 1, -1, 8, (180, 160, 140), 2);
                    }
                    if just_cross {
                        // Wall-Jump kicking off left wall!
                        self.vy = -Fixed::from_fraction(9, 2);
                        self.vx = Fixed::from_fraction(13, 4);
                        self.facing_right = true;
                        self.state = PlayerState::Jumping;
                        self.wall_slide_side = 0;
                        AudioManager::play_jump();
                        entities.spawn_particle(px + 2, py + 12, 2, 1, 10, (255, 255, 255), 2);
                    }
                } else if right_wall && btn_right {
                    is_wall_sliding = true;
                    self.state = PlayerState::WallSlide;
                    self.wall_slide_side = 1;
                    self.facing_right = false; // Tail flat against right wall, looking left
                    self.vy = Fixed::from_fraction(1, 2);
                    if self.anim_frame % 4 == 0 {
                        entities.spawn_particle(px + 14, py + 10, -1, -1, 8, (180, 160, 140), 2);
                    }
                    if just_cross {
                        // Wall-Jump kicking off right wall!
                        self.vy = -Fixed::from_fraction(9, 2);
                        self.vx = -Fixed::from_fraction(13, 4);
                        self.facing_right = false;
                        self.state = PlayerState::Jumping;
                        self.wall_slide_side = 0;
                        AudioManager::play_jump();
                        entities.spawn_particle(px + 14, py + 12, -2, 1, 10, (255, 255, 255), 2);
                    }
                }
            }

            if !is_wall_sliding {
                self.wall_slide_side = 0;

                // Normal walking / running movement
                let accel = Fixed::from_fraction(1, 4);
                let max_speed = Fixed::from_fraction(9, 4);

                if btn_left {
                    self.facing_right = false;
                    self.vx -= accel;
                    if self.vx < -max_speed {
                        self.vx = -max_speed;
                    }
                    if self.on_ground && self.waddle_audio_cooldown == 0 {
                        AudioManager::play_waddle();
                        self.waddle_audio_cooldown = 18;
                    }
                } else if btn_right {
                    self.facing_right = true;
                    self.vx += accel;
                    if self.vx > max_speed {
                        self.vx = max_speed;
                    }
                    if self.on_ground && self.waddle_audio_cooldown == 0 {
                        AudioManager::play_waddle();
                        self.waddle_audio_cooldown = 18;
                    }
                } else {
                    // Ground friction
                    self.vx = self.vx * Fixed::from_fraction(75, 100);
                }

                // Jump / flutter
                if just_cross && self.on_ground {
                    self.vy = -Fixed::from_fraction(9, 2); // jump impulse
                    self.on_ground = false;
                    self.state = PlayerState::Jumping;
                    AudioManager::play_jump();
                } else if just_cross && !self.on_ground && self.vy > Fixed::ZERO {
                    // Flutter kick mid-air!
                    self.vy = -Fixed::from_fraction(2, 1);
                    self.state = PlayerState::Fluttering;
                    AudioManager::play_waddle();
                    entities.spawn_particle(px + 8, py + 12, 0, 1, 10, (200, 200, 255), 1);
                }
            }
        }

        // Gravity
        if self.state != PlayerState::WallSlide && self.state != PlayerState::SpurStomp {
            let gravity = Fixed::from_fraction(1, 4);
            let max_fall = Fixed::from_int(5);
            self.vy += gravity;
            if self.vy > max_fall {
                self.vy = max_fall;
            }
        }

        // Apply velocities and tile collision
        self.apply_movement_land(level, entities);

        // Update stance
        if self.slide_timer == 0 && self.whip_timer == 0 && self.state != PlayerState::SpurStomp && self.state != PlayerState::WallSlide {
            if !self.on_ground {
                if self.state != PlayerState::HydroBreach {
                    if self.vy < Fixed::ZERO {
                        self.state = PlayerState::Jumping;
                    } else {
                        self.state = PlayerState::Fluttering;
                    }
                }
            } else if self.vx.abs() > Fixed::from_fraction(1, 4) {
                self.state = PlayerState::Running;
            } else {
                self.state = PlayerState::Standing;
            }
        }
    }

    fn update_swimming(
        &mut self,
        buttons: ButtonState,
        _prev_buttons: ButtonState,
        level: &Level,
        entities: &mut EntityManager,
    ) {
        self.state = PlayerState::Swimming;

        let btn_left = buttons.is_held(button::LEFT);
        let btn_right = buttons.is_held(button::RIGHT);
        let btn_up = buttons.is_held(button::UP);
        let btn_down = buttons.is_held(button::DOWN);
        let btn_cross = buttons.is_held(button::CROSS);

        let swim_speed = if btn_cross {
            Fixed::from_int(3) // Fast paddle
        } else {
            Fixed::from_fraction(3, 2)
        };

        if btn_left {
            self.facing_right = false;
            self.vx = -swim_speed;
        } else if btn_right {
            self.facing_right = true;
            self.vx = swim_speed;
        } else {
            self.vx = self.vx * Fixed::from_fraction(85, 100);
        }

        if btn_up {
            self.vy = -swim_speed;
        } else if btn_down {
            self.vy = swim_speed;
        } else {
            // Slight natural buoyancy upward in water
            self.vy = self.vy * Fixed::from_fraction(85, 100) - Fixed::from_fraction(1, 16);
        }

        // Spawn swimming bubbles
        if self.anim_frame % 5 == 0 {
            let px = self.x.to_int();
            let py = self.y.to_int();
            let tail_x = if self.facing_right { px - 2 } else { px + 16 };
            entities.spawn_particle(tail_x, py + 8, 0, -1, 15, (180, 230, 255), 2);
        }

        // Apply movement
        self.x += self.vx;
        self.y += self.vy;

        // Keep inside level bounds
        let px = self.x.to_int();
        let py = self.y.to_int();
        if px < 0 {
            self.x = Fixed::ZERO;
        }
        if py < 0 {
            self.y = Fixed::ZERO;
        }

        // Check solid collision while swimming
        if level.is_solid_at(px + 4, py + 8) || level.is_solid_at(px + 12, py + 8) {
            self.x -= self.vx;
            self.vx = Fixed::ZERO;
        }
        if level.is_solid_at(px + 8, py + 14) {
            self.y -= self.vy;
            self.vy = Fixed::ZERO;
        }
    }

    fn apply_movement_land(&mut self, level: &Level, entities: &mut EntityManager) {
        // Horizontal movement
        self.x += self.vx;
        let px = self.x.to_int();
        let py = self.y.to_int();

        let hitbox_h = if self.state == PlayerState::BellySlide { 8 } else { 14 };

        if self.vx > Fixed::ZERO {
            if level.is_solid_at(px + 14, py + 4) || level.is_solid_at(px + 14, py + hitbox_h) {
                self.x = Fixed::from_int((px + 14) / TILE_SIZE * TILE_SIZE - 15);
                self.vx = Fixed::ZERO;
            }
        } else if self.vx < Fixed::ZERO {
            if level.is_solid_at(px + 2, py + 4) || level.is_solid_at(px + 2, py + hitbox_h) {
                self.x = Fixed::from_int(((px + 2) / TILE_SIZE + 1) * TILE_SIZE);
                self.vx = Fixed::ZERO;
            }
        }

        // Vertical movement
        self.y += self.vy;
        let px = self.x.to_int();
        let py = self.y.to_int();

        if self.vy > Fixed::ZERO {
            // Falling / landing check
            let left_solid = level.is_solid_at(px + 4, py + 16);
            let right_solid = level.is_solid_at(px + 12, py + 16);

            let on_slope = level.get_tile((px + 8) / TILE_SIZE, (py + 16) / TILE_SIZE) == TileType::SlopeDown;

            if left_solid || right_solid {
                self.y = Fixed::from_int((py + 16) / TILE_SIZE * TILE_SIZE - 16);
                self.vy = Fixed::ZERO;
                self.on_ground = true;

                if self.state == PlayerState::SpurStomp {
                    self.state = PlayerState::Standing;
                    AudioManager::play_hit();
                    entities.spawn_particle(px + 4, py + 14, -3, -1, 15, (230, 180, 255), 3);
                    entities.spawn_particle(px + 12, py + 14, 3, -1, 15, (230, 180, 255), 3);
                } else if self.state == PlayerState::HydroBreach {
                    self.state = PlayerState::Standing;
                }
            } else if on_slope {
                self.on_ground = true;
                // Slopes accelerate belly slide!
                if self.state == PlayerState::BellySlide {
                    self.vx += Fixed::from_fraction(1, 4);
                }
            } else {
                self.on_ground = false;
            }
        } else if self.vy < Fixed::ZERO {
            // Ceiling check
            if level.is_solid_at(px + 4, py) || level.is_solid_at(px + 12, py) {
                self.y = Fixed::from_int(((py) / TILE_SIZE + 1) * TILE_SIZE);
                self.vy = Fixed::ZERO;
                AudioManager::play_hit();
            }
            self.on_ground = false;
        }
    }

    fn check_collectibles(&mut self, entities: &mut EntityManager) {
        let px = self.x.to_int();
        let py = self.y.to_int();
        let mut spawn_pos = None;

        for i in 0..crate::entities::MAX_COLLECTIBLES {
            let c = &mut entities.collectibles[i];
            if !c.active {
                continue;
            }
            let dx = (px + 8) - (c.x + 8);
            let dy = (py + 8) - (c.y + 8);
            if dx.abs() < 14 && dy.abs() < 14 {
                c.active = false;
                match c.kind {
                    crate::entities::CollectibleType::Yabby => {
                        self.yabbies_collected += 1;
                        self.score += 100;
                        if self.health < self.max_health {
                            self.health += 1;
                        }
                        AudioManager::play_yabby();
                    }
                    crate::entities::CollectibleType::LetterPage => {
                        self.score += 250;
                        AudioManager::play_fanfare();
                    }
                }
                spawn_pos = Some((c.x + 8, c.y + 8));
            }
        }

        if let Some((sx, sy)) = spawn_pos {
            entities.spawn_particle(sx, sy, 0, -1, 15, (255, 230, 80), 2);
        }
    }

    fn check_enemies(&mut self, entities: &mut EntityManager) {
        let px = self.x.to_int();
        let py = self.y.to_int();

        let is_whipping = self.whip_timer > 0;
        let is_stomping = self.state == PlayerState::SpurStomp;
        let whip_reach_x = if self.facing_right { px + 22 } else { px - 6 };
        let mut hit_pos = None;
        let mut stomp_pos = None;
        let mut electro_spark_pos = None;

        for i in 0..crate::entities::MAX_ENEMIES {
            let e = &mut entities.enemies[i];
            if !e.active {
                continue;
            }
            let ex = e.x.to_int();
            let ey = e.y.to_int();

            // Electro-reception radar pulse: disorients/stuns nearby enemies!
            if self.electro_timer > 0 {
                let edx = (px + 8) - (ex + 8);
                let edy = (py + 8) - (ey + 8);
                if edx.abs() < 42 && edy.abs() < 42 && e.stun_timer == 0 {
                    e.stun_timer = 25;
                    electro_spark_pos = Some((ex + 8, ey + 4));
                }
            }

            // Venom-Spur Stomp strike!
            if is_stomping {
                let sdx = (px + 8) - (ex + 8);
                let sdy = (py + 12) - (ey + 8);
                if sdx.abs() < 16 && sdy.abs() < 14 {
                    e.stun_timer = 90;
                    AudioManager::play_hit();
                    stomp_pos = Some((ex + 8, ey + 6));
                    self.score += 200;
                    // Rebound upward!
                    self.vy = -Fixed::from_fraction(11, 2);
                    self.state = PlayerState::Jumping;
                    continue;
                }
            }

            // Tail whip strike check
            if is_whipping {
                let whip_dx = (whip_reach_x) - (ex + 8);
                let whip_dy = (py + 8) - (ey + 8);
                if whip_dx.abs() < 16 && whip_dy.abs() < 14 {
                    e.stun_timer = 40; // Stun enemy!
                    AudioManager::play_hit();
                    hit_pos = Some((ex + 8, ey + 8));
                    self.score += 50;
                    continue;
                }
            }

            // Body collision check
            let dx = (px + 8) - (ex + 8);
            let dy = (py + 8) - (ey + 8);
            if dx.abs() < 12 && dy.abs() < 12 {
                if e.stun_timer == 0 {
                    self.take_damage(1);
                    // Knockback
                    self.vx = if dx < 0 { -Fixed::from_int(2) } else { Fixed::from_int(2) };
                    self.vy = -Fixed::from_int(2);
                }
            }
        }

        if let Some((sx, sy)) = electro_spark_pos {
            entities.spawn_particle(sx, sy, 0, -1, 12, (100, 255, 255), 1);
        }
        if let Some((sx, sy)) = stomp_pos {
            entities.spawn_particle(sx, sy, -2, -2, 18, (255, 100, 240), 3);
            entities.spawn_particle(sx, sy, 2, -2, 18, (255, 100, 240), 3);
        }
        if let Some((sx, sy)) = hit_pos {
            entities.spawn_particle(sx, sy, 1, -1, 15, (255, 100, 100), 2);
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
        self.invuln_timer = 60; // 1 second invulnerability flash
        AudioManager::play_hit();
    }
}
