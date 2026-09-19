//! 3D Tactical Entities: Sentries with vision cones, Cypher drones,
//! searchlights, Soliton Radar detection, and tactical collectibles.

use crate::audio::AudioManager;
use crate::level::{Act, Level, TILE_SZ};
use psx_gte_core::transform::{cos_1_3_12, sin_1_3_12};

pub const MAX_SENTRIES: usize = 6;
pub const MAX_DRONES: usize = 3;
pub const MAX_SEARCHLIGHTS: usize = 3;
pub const MAX_COLLECTIBLES: usize = 16;
pub const MAX_PARTICLES: usize = 24;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum AlertState {
    Sneaking,
    Caution(u16),
    Alert(u16),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SentryState {
    Patrolling,
    Investigating,
    AlertChase,
    Stunned,
}

#[derive(Copy, Clone, Debug)]
pub struct Sentry {
    pub active: bool,
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub angle: u16, // 0..256
    pub state: SentryState,
    pub waypoints: [(i32, i32); 4],
    pub waypoint_count: usize,
    pub current_waypoint: usize,
    pub wait_timer: u16,
    pub stun_timer: u16,
    pub see_player: bool,
}

impl Sentry {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            z: 0,
            y: 0,
            angle: 0,
            state: SentryState::Patrolling,
            waypoints: [(0, 0); 4],
            waypoint_count: 0,
            current_waypoint: 0,
            wait_timer: 0,
            stun_timer: 0,
            see_player: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Drone {
    pub active: bool,
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub angle: u16,
    pub stun_timer: u16,
}

impl Drone {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            z: 0,
            y: -40,
            angle: 0,
            stun_timer: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Searchlight {
    pub active: bool,
    pub base_x: i32,
    pub base_z: i32,
    pub beam_x: i32,
    pub beam_z: i32,
    pub sweep_angle: u16,
    pub radius: i32,
}

impl Searchlight {
    pub const fn empty() -> Self {
        Self {
            active: false,
            base_x: 0,
            base_z: 0,
            beam_x: 0,
            beam_z: 0,
            sweep_angle: 0,
            radius: 40,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CollectibleType {
    YabbyRation,   // Restores health +1 heart
    ChaffBattery,  // Powers electro-bill
    LetterPage,    // Intelligence intel from parents
    BuriedYabby,   // Hidden ration, requires electro pulse
}

#[derive(Copy, Clone, Debug)]
pub struct Collectible {
    pub active: bool,
    pub kind: CollectibleType,
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub revealed: bool,
}

impl Collectible {
    pub const fn empty() -> Self {
        Self {
            active: false,
            kind: CollectibleType::YabbyRation,
            x: 0,
            z: 0,
            y: 0,
            revealed: true,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Particle3D {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub life: u8,
    pub color: (u8, u8, u8),
    pub size: u8,
}

impl Particle3D {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            vx: 0,
            vy: 0,
            vz: 0,
            life: 0,
            color: (255, 255, 255),
            size: 2,
        }
    }
}

pub struct EntityManager {
    pub alert_state: AlertState,
    pub sentries: [Sentry; MAX_SENTRIES],
    pub drones: [Drone; MAX_DRONES],
    pub searchlights: [Searchlight; MAX_SEARCHLIGHTS],
    pub collectibles: [Collectible; MAX_COLLECTIBLES],
    pub particles: [Particle3D; MAX_PARTICLES],
    pub frame: u16,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            alert_state: AlertState::Sneaking,
            sentries: [Sentry::empty(); MAX_SENTRIES],
            drones: [Drone::empty(); MAX_DRONES],
            searchlights: [Searchlight::empty(); MAX_SEARCHLIGHTS],
            collectibles: [Collectible::empty(); MAX_COLLECTIBLES],
            particles: [Particle3D::empty(); MAX_PARTICLES],
            frame: 0,
        }
    }

    pub fn spawn_particle(&mut self, x: i32, y: i32, z: i32, vx: i16, vy: i16, vz: i16, life: u8, color: (u8, u8, u8), size: u8) {
        for p in self.particles.iter_mut() {
            if !p.active {
                *p = Particle3D {
                    active: true,
                    x,
                    y,
                    z,
                    vx,
                    vy,
                    vz,
                    life,
                    color,
                    size,
                };
                break;
            }
        }
    }

    pub fn trigger_alert(&mut self) {
        if !matches!(self.alert_state, AlertState::Alert(_)) {
            self.alert_state = AlertState::Alert(600); // 10 seconds of RED ALERT
            AudioManager::play_metal(); // Alert chord!
        } else if let AlertState::Alert(ref mut timer) = self.alert_state {
            *timer = 600;
        }
    }

    pub fn load_act(&mut self, act: Act) {
        self.alert_state = AlertState::Sneaking;
        self.sentries = [Sentry::empty(); MAX_SENTRIES];
        self.drones = [Drone::empty(); MAX_DRONES];
        self.searchlights = [Searchlight::empty(); MAX_SEARCHLIGHTS];
        self.collectibles = [Collectible::empty(); MAX_COLLECTIBLES];
        self.particles = [Particle3D::empty(); MAX_PARTICLES];

        match act {
            Act::Act1Sanctuary => self.load_act1(),
            Act::Act2Bushland => self.load_act2(),
            Act::Act3City => self.load_act3(),
            Act::Act4Ocean => self.load_act4(),
        }
    }

    fn load_act1(&mut self) {
        // Sentries patrolling compound corridors
        self.spawn_sentry(0, 5 * TILE_SZ, 3 * TILE_SZ, &[
            (5 * TILE_SZ, 3 * TILE_SZ),
            (5 * TILE_SZ, 12 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 15 * TILE_SZ, 4 * TILE_SZ, &[
            (15 * TILE_SZ, 4 * TILE_SZ),
            (20 * TILE_SZ, 4 * TILE_SZ),
        ]);
        self.spawn_sentry(2, 17 * TILE_SZ, 18 * TILE_SZ, &[
            (17 * TILE_SZ, 18 * TILE_SZ),
            (17 * TILE_SZ, 12 * TILE_SZ),
        ]);

        // Searchlight watchtowers
        self.searchlights[0] = Searchlight {
            active: true,
            base_x: 8 * TILE_SZ,
            base_z: 6 * TILE_SZ,
            beam_x: 8 * TILE_SZ,
            beam_z: 6 * TILE_SZ,
            sweep_angle: 0,
            radius: 50,
        };
        self.searchlights[1] = Searchlight {
            active: true,
            base_x: 16 * TILE_SZ,
            base_z: 14 * TILE_SZ,
            beam_x: 16 * TILE_SZ,
            beam_z: 14 * TILE_SZ,
            sweep_angle: 128,
            radius: 55,
        };

        // Collectibles
        self.spawn_item(0, CollectibleType::YabbyRation, 4 * TILE_SZ, 7 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::LetterPage, 11 * TILE_SZ, 3 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 14 * TILE_SZ, 18 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::BuriedYabby, 3 * TILE_SZ, 12 * TILE_SZ, false);
    }

    fn load_act2(&mut self) {
        self.spawn_sentry(0, 7 * TILE_SZ, 5 * TILE_SZ, &[
            (7 * TILE_SZ, 5 * TILE_SZ),
            (14 * TILE_SZ, 5 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 8 * TILE_SZ, 18 * TILE_SZ, &[
            (8 * TILE_SZ, 18 * TILE_SZ),
            (15 * TILE_SZ, 18 * TILE_SZ),
        ]);

        // Flying surveillance drone scanning the creek
        self.drones[0] = Drone {
            active: true,
            x: 10 * TILE_SZ,
            z: 11 * TILE_SZ,
            y: -40,
            angle: 0,
            stun_timer: 0,
        };

        // Collectibles
        self.spawn_item(0, CollectibleType::YabbyRation, 5 * TILE_SZ, 4 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::LetterPage, 11 * TILE_SZ, 11 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::ChaffBattery, 18 * TILE_SZ, 15 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::BuriedYabby, 14 * TILE_SZ, 9 * TILE_SZ, false);
    }

    fn load_act3(&mut self) {
        self.spawn_sentry(0, 10 * TILE_SZ, 5 * TILE_SZ, &[
            (10 * TILE_SZ, 5 * TILE_SZ),
            (10 * TILE_SZ, 19 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 14 * TILE_SZ, 15 * TILE_SZ, &[
            (14 * TILE_SZ, 15 * TILE_SZ),
            (20 * TILE_SZ, 15 * TILE_SZ),
        ]);

        self.drones[0] = Drone {
            active: true,
            x: 6 * TILE_SZ,
            z: 12 * TILE_SZ,
            y: -45,
            angle: 0,
            stun_timer: 0,
        };
        self.drones[1] = Drone {
            active: true,
            x: 17 * TILE_SZ,
            z: 12 * TILE_SZ,
            y: -45,
            angle: 128,
            stun_timer: 0,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, 3 * TILE_SZ, 12 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::LetterPage, 9 * TILE_SZ, 12 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::ChaffBattery, 16 * TILE_SZ, 8 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::BuriedYabby, 19 * TILE_SZ, 12 * TILE_SZ, false);
    }

    fn load_act4(&mut self) {
        self.spawn_sentry(0, 6 * TILE_SZ, 5 * TILE_SZ, &[
            (6 * TILE_SZ, 5 * TILE_SZ),
            (15 * TILE_SZ, 5 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 10 * TILE_SZ, 11 * TILE_SZ, &[
            (10 * TILE_SZ, 11 * TILE_SZ),
            (16 * TILE_SZ, 11 * TILE_SZ),
        ]);

        // Coastline searchlight sweeping the water
        self.searchlights[0] = Searchlight {
            active: true,
            base_x: 10 * TILE_SZ,
            base_z: 13 * TILE_SZ,
            beam_x: 10 * TILE_SZ,
            beam_z: 16 * TILE_SZ,
            sweep_angle: 0,
            radius: 65,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, 4 * TILE_SZ, 4 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::LetterPage, 14 * TILE_SZ, 4 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 12 * TILE_SZ, 18 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::BuriedYabby, 18 * TILE_SZ, 18 * TILE_SZ, false);
    }

    fn spawn_sentry(&mut self, idx: usize, x: i32, z: i32, waypoints: &[(i32, i32)]) {
        if idx >= MAX_SENTRIES {
            return;
        }
        let mut pts = [(0, 0); 4];
        let count = waypoints.len().min(4);
        for i in 0..count {
            pts[i] = waypoints[i];
        }
        self.sentries[idx] = Sentry {
            active: true,
            x,
            z,
            y: 0,
            angle: 0,
            state: SentryState::Patrolling,
            waypoints: pts,
            waypoint_count: count,
            current_waypoint: 0,
            wait_timer: 0,
            stun_timer: 0,
            see_player: false,
        };
    }

    fn spawn_item(&mut self, idx: usize, kind: CollectibleType, x: i32, z: i32, revealed: bool) {
        if idx < MAX_COLLECTIBLES {
            self.collectibles[idx] = Collectible {
                active: true,
                kind,
                x,
                z,
                y: -12,
                revealed,
            };
        }
    }

    /// Update sentry AI, vision cone detection, and searchlights.
    pub fn update(
        &mut self,
        player_x: i32,
        player_z: i32,
        player_crawling: bool,
        player_submerged: bool,
        level: &Level,
    ) {
        self.frame = self.frame.wrapping_add(1);

        // Update Alert countdown state
        match self.alert_state {
            AlertState::Sneaking => {}
            AlertState::Caution(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                } else {
                    self.alert_state = AlertState::Sneaking;
                }
            }
            AlertState::Alert(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                } else {
                    self.alert_state = AlertState::Caution(300); // Drop to Caution
                }
            }
        }

        let is_in_alert = matches!(self.alert_state, AlertState::Alert(_));

        // Update Searchlights
        for s in self.searchlights.iter_mut() {
            if !s.active {
                continue;
            }
            s.sweep_angle = s.sweep_angle.wrapping_add(1);
            let offset_x = (cos_1_3_12(s.sweep_angle) as i32 * s.radius) >> 12;
            let offset_z = (sin_1_3_12(s.sweep_angle) as i32 * s.radius) >> 12;
            s.beam_x = s.base_x + offset_x;
            s.beam_z = s.base_z + offset_z;

            // Detection check: player standing inside beam and not submerged
            if !player_submerged {
                let dx = (player_x - s.beam_x).abs();
                let dz = (player_z - s.beam_z).abs();
                if dx < 24 && dz < 24 {
                    // Searchlight spotted Platty!
                    self.alert_state = AlertState::Alert(600);
                    AudioManager::play_metal();
                }
            }
        }

        // Update Drones
        for d in self.drones.iter_mut() {
            if !d.active {
                continue;
            }
            if d.stun_timer > 0 {
                d.stun_timer -= 1;
                continue;
            }
            d.angle = d.angle.wrapping_add(1);
            // Drone flies circular pattern
            d.x += (cos_1_3_12(d.angle) as i32 * 2) >> 12;
            d.z += (sin_1_3_12(d.angle) as i32 * 2) >> 12;

            if !player_submerged && !player_crawling {
                let dx = (player_x - d.x).abs();
                let dz = (player_z - d.z).abs();
                if dx < 36 && dz < 36 {
                    self.alert_state = AlertState::Alert(600);
                    AudioManager::play_metal();
                }
            }
        }

        // Update Sentries
        for s in self.sentries.iter_mut() {
            if !s.active {
                continue;
            }

            if s.stun_timer > 0 {
                s.stun_timer -= 1;
                s.state = SentryState::Stunned;
                s.see_player = false;
                continue;
            }

            // Check vision cone detection
            let mut detected = false;
            if !player_submerged {
                let dx = player_x - s.x;
                let dz = player_z - s.z;
                let dist_sq = dx * dx + dz * dz;

                // Vision range: 200 world units (~3 tiles)
                if dist_sq < (200 * 200) {
                    // Check angle relative to sentry facing
                    // Sentry forward vector:
                    let fwd_x = cos_1_3_12(s.angle) as i32;
                    let fwd_z = sin_1_3_12(s.angle) as i32;
                    let dot = (dx * fwd_x + dz * fwd_z) >> 12;

                    // Inside 60-degree field of view (dot > 0.6 in 1.3.12 ≈ 2450)
                    if dot > 2400 {
                        // Check if player is crawling through tall grass or behind crate
                        let in_grass = level.is_tall_grass_at(player_x, player_z);
                        if player_crawling && in_grass {
                            // Camouflage! Undetected
                            detected = false;
                        } else {
                            // Line of sight check (midpoint solid check)
                            let mid_x = (s.x + player_x) / 2;
                            let mid_z = (s.z + player_z) / 2;
                            if !level.is_solid_at(mid_x, mid_z, false) {
                                detected = true;
                            }
                        }
                    }
                }
            }

            s.see_player = detected;
            if detected {
                s.state = SentryState::AlertChase;
                self.alert_state = AlertState::Alert(600);
            }

            // Sentry movement AI
            match s.state {
                SentryState::Stunned => {}
                SentryState::AlertChase => {
                    // Chase player
                    let dx = player_x - s.x;
                    let dz = player_z - s.z;
                    if dx.abs() > 8 {
                        s.x += if dx > 0 { 2 } else { -2 };
                    }
                    if dz.abs() > 8 {
                        s.z += if dz > 0 { 2 } else { -2 };
                    }
                    // Aim facing towards player
                    if dx > 0 {
                        s.angle = 0;
                    } else if dx < 0 {
                        s.angle = 128;
                    } else if dz > 0 {
                        s.angle = 64;
                    } else {
                        s.angle = 192;
                    }
                    if !is_in_alert {
                        s.state = SentryState::Patrolling;
                    }
                }
                SentryState::Patrolling | SentryState::Investigating => {
                    if s.waypoint_count > 0 {
                        let target = s.waypoints[s.current_waypoint];
                        let dx = target.0 - s.x;
                        let dz = target.1 - s.z;

                        if dx.abs() < 4 && dz.abs() < 4 {
                            // Arrived at waypoint
                            s.wait_timer += 1;
                            if s.wait_timer > 60 {
                                s.wait_timer = 0;
                                s.current_waypoint = (s.current_waypoint + 1) % s.waypoint_count;
                            }
                        } else {
                            let speed = if is_in_alert { 2 } else { 1 };
                            if dx.abs() > 2 {
                                s.x += if dx > 0 { speed } else { -speed };
                                s.angle = if dx > 0 { 0 } else { 128 };
                            }
                            if dz.abs() > 2 {
                                s.z += if dz > 0 { speed } else { -speed };
                                s.angle = if dz > 0 { 64 } else { 192 };
                            }
                        }
                    }
                }
            }
        }

        // Update 3D particles
        for p in self.particles.iter_mut() {
            if !p.active {
                continue;
            }
            p.x += p.vx as i32;
            p.y += p.vy as i32;
            p.z += p.vz as i32;
            if p.life > 0 {
                p.life -= 1;
            } else {
                p.active = false;
            }
        }
    }
}
