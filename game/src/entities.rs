//! 3D Tactical Entities & Multi-Theme Mechanics:
//! Act 1: Sentries with vision cones, Cypher drones, searchlights, Soliton Radar.
//! Act 2: Yarra River 5-lane obstacles (Trees, branches, snakes, spiders, tubers, paddle boarders, swimmers, koalas).
//! Act 3: Melbourne City Frogger traffic (Taxis, sedans, trams, trucks, sports cars).
//! Act 4: Coastal Beach 3D platformer (Stepped rock ledges, bouncing parasols, beach crabs).

use crate::audio::{AudioManager, BgmTrack};
use crate::level::{Act, CellType, Level, TILE_SZ};
use psx_gte_core::transform::{cos_1_3_12, sin_1_3_12};

pub const MAX_SENTRIES: usize = 6;
pub const MAX_DRONES: usize = 3;
pub const MAX_SEARCHLIGHTS: usize = 3;
pub const MAX_COLLECTIBLES: usize = 16;
pub const MAX_PARTICLES: usize = 24;
pub const MAX_RIVER_OBSTACLES: usize = 16;
pub const MAX_VEHICLES: usize = 14;
pub const MAX_BEACH_PLATFORMS: usize = 12;
pub const MAX_BEACH_CRABS: usize = 6;

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
    pub attack_cooldown: u8,
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
            attack_cooldown: 0,
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
pub enum MechState {
    Patrolling,
    Targeting(u16),
    Stomping(u16),
    Venting(u16),
    Defeated(u16),
}

#[derive(Copy, Clone, Debug)]
pub struct SearchlightMech {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub angle: u16,
    pub health: u8,
    pub max_health: u8,
    pub shield_active: bool,
    pub state: MechState,
    pub left_light_angle: u16,
    pub right_light_angle: u16,
    pub left_beam_x: i32,
    pub left_beam_z: i32,
    pub right_beam_x: i32,
    pub right_beam_z: i32,
    pub shockwave_radius: i32,
    pub shockwave_active: bool,
    pub walk_dir: i32,
    pub hit_timer: u8,
    pub stomp_cooldown: u16,
    pub leg_anim: u8,
}

impl SearchlightMech {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            angle: 0,
            health: 4,
            max_health: 4,
            shield_active: true,
            state: MechState::Patrolling,
            left_light_angle: 0,
            right_light_angle: 128,
            left_beam_x: 0,
            left_beam_z: 0,
            right_beam_x: 0,
            right_beam_z: 0,
            shockwave_radius: 0,
            shockwave_active: false,
            walk_dir: 1,
            hit_timer: 0,
            stomp_cooldown: 180,
            leg_anim: 0,
        }
    }

    pub fn is_defeated(&self) -> bool {
        matches!(self.state, MechState::Defeated(0)) || (self.health == 0 && !self.active)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PowerConduit {
    pub active: bool,
    pub destroyed: bool,
    pub x: i32,
    pub z: i32,
    pub health: u8,
    pub spark_timer: u8,
}

impl PowerConduit {
    pub const fn empty() -> Self {
        Self {
            active: false,
            destroyed: false,
            x: 0,
            z: 0,
            health: 3,
            spark_timer: 0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CollectibleType {
    YabbyRation,   // Restores health +1 heart
    ChaffBattery,  // Powers electro-bill
    LetterPage,    // Intelligence intel from parents
    BuriedYabby,   // Hidden ration, requires electro pulse
    StarYabby,     // Mario 64 star yabby in Act 4
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RiverObstacleType {
    TreeLog,
    LowBranch,
    TigerSnake,
    GiantSpider,
    RiverTuber,
    PaddleBoarder,
    Swimmer,
    Koala,
}

#[derive(Copy, Clone, Debug)]
pub struct RiverObstacle {
    pub active: bool,
    pub kind: RiverObstacleType,
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub speed: i32,
}

impl RiverObstacle {
    pub const fn empty() -> Self {
        Self {
            active: false,
            kind: RiverObstacleType::TreeLog,
            x: 0,
            z: 0,
            y: 0,
            speed: 0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VehicleType {
    Taxi,
    Sedan,
    Tram,
    Truck,
    SportsCar,
}

#[derive(Copy, Clone, Debug)]
pub struct CityVehicle {
    pub active: bool,
    pub kind: VehicleType,
    pub x: i32,
    pub z: i32,
    pub vx: i32,
    pub length: i32,
    pub color: (u8, u8, u8),
}

impl CityVehicle {
    pub const fn empty() -> Self {
        Self {
            active: false,
            kind: VehicleType::Sedan,
            x: 0,
            z: 0,
            vx: 0,
            length: 32,
            color: (60, 100, 200),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BeachPlatform {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    pub d: i32,
    pub is_parasol: bool,
}

impl BeachPlatform {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            w: 48,
            h: 24,
            d: 48,
            is_parasol: false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BeachCrab {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub min_x: i32,
    pub max_x: i32,
    pub vx: i32,
}

impl BeachCrab {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            min_x: 0,
            max_x: 0,
            vx: 1,
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
    pub boss_mech: SearchlightMech,
    pub power_conduits: [PowerConduit; 3],
    pub collectibles: [Collectible; MAX_COLLECTIBLES],
    pub particles: [Particle3D; MAX_PARTICLES],
    pub river_obstacles: [RiverObstacle; MAX_RIVER_OBSTACLES],
    pub vehicles: [CityVehicle; MAX_VEHICLES],
    pub beach_platforms: [BeachPlatform; MAX_BEACH_PLATFORMS],
    pub beach_crabs: [BeachCrab; MAX_BEACH_CRABS],
    pub frame: u16,
    pub river_distance: i32,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            alert_state: AlertState::Sneaking,
            sentries: [Sentry::empty(); MAX_SENTRIES],
            drones: [Drone::empty(); MAX_DRONES],
            searchlights: [Searchlight::empty(); MAX_SEARCHLIGHTS],
            boss_mech: SearchlightMech::empty(),
            power_conduits: [PowerConduit::empty(); 3],
            collectibles: [Collectible::empty(); MAX_COLLECTIBLES],
            particles: [Particle3D::empty(); MAX_PARTICLES],
            river_obstacles: [RiverObstacle::empty(); MAX_RIVER_OBSTACLES],
            vehicles: [CityVehicle::empty(); MAX_VEHICLES],
            beach_platforms: [BeachPlatform::empty(); MAX_BEACH_PLATFORMS],
            beach_crabs: [BeachCrab::empty(); MAX_BEACH_CRABS],
            frame: 0,
            river_distance: 0,
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
            AudioManager::play_alert();
            AudioManager::set_bgm(BgmTrack::Alert);
        } else if let AlertState::Alert(ref mut timer) = self.alert_state {
            *timer = 600;
        }
    }

    pub fn load_act(&mut self, act: Act) {
        self.alert_state = AlertState::Sneaking;
        self.sentries = [Sentry::empty(); MAX_SENTRIES];
        self.drones = [Drone::empty(); MAX_DRONES];
        self.searchlights = [Searchlight::empty(); MAX_SEARCHLIGHTS];
        self.boss_mech = SearchlightMech::empty();
        self.power_conduits = [PowerConduit::empty(); 3];
        self.collectibles = [Collectible::empty(); MAX_COLLECTIBLES];
        self.particles = [Particle3D::empty(); MAX_PARTICLES];
        self.river_obstacles = [RiverObstacle::empty(); MAX_RIVER_OBSTACLES];
        self.vehicles = [CityVehicle::empty(); MAX_VEHICLES];
        self.beach_platforms = [BeachPlatform::empty(); MAX_BEACH_PLATFORMS];
        self.beach_crabs = [BeachCrab::empty(); MAX_BEACH_CRABS];
        self.river_distance = 0;

        match act {
            Act::Act1Sanctuary => self.load_act1(),
            Act::Act1Boss => self.load_act1_boss(),
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

    fn load_act1_boss(&mut self) {
        self.boss_mech = SearchlightMech {
            active: true,
            x: 12 * TILE_SZ,
            y: 0,
            z: 6 * TILE_SZ,
            angle: 0,
            health: 4,
            max_health: 4,
            shield_active: true,
            state: MechState::Patrolling,
            left_light_angle: 0,
            right_light_angle: 128,
            left_beam_x: 10 * TILE_SZ,
            left_beam_z: 11 * TILE_SZ,
            right_beam_x: 14 * TILE_SZ,
            right_beam_z: 11 * TILE_SZ,
            shockwave_radius: 0,
            shockwave_active: false,
            walk_dir: 1,
            hit_timer: 0,
            stomp_cooldown: 150,
            leg_anim: 0,
        };

        // 3 Destructible Power Conduits
        // 0: West generator bay (gx 5, gz 9)
        self.power_conduits[0] = PowerConduit {
            active: true,
            destroyed: false,
            x: 5 * TILE_SZ + 32,
            z: 9 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };
        // 1: East generator bay (gx 19, gz 9)
        self.power_conduits[1] = PowerConduit {
            active: true,
            destroyed: false,
            x: 19 * TILE_SZ + 32,
            z: 9 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };
        // 2: South generator bay (gx 12, gz 17)
        self.power_conduits[2] = PowerConduit {
            active: true,
            destroyed: false,
            x: 12 * TILE_SZ + 32,
            z: 17 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };

        // Emergency Yabby Rations in corner trenches
        self.spawn_item(0, CollectibleType::YabbyRation, 3 * TILE_SZ + 32, 9 * TILE_SZ + 32, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 20 * TILE_SZ + 32, 9 * TILE_SZ + 32, true);
    }

    fn load_act2(&mut self) {
        // 5-Lane Yarra River Runner Obstacles
        // Lanes: 0=gx9, 1=gx10, 2=gx11, 3=gx12, 4=gx13
        let lane_x = [
            9 * TILE_SZ + 32,
            10 * TILE_SZ + 32,
            11 * TILE_SZ + 32,
            12 * TILE_SZ + 32,
            13 * TILE_SZ + 32,
        ];

        let obstacles = [
            (0, RiverObstacleType::TreeLog, 5 * TILE_SZ, 0),
            (1, RiverObstacleType::RiverTuber, 4 * TILE_SZ, 1),
            (2, RiverObstacleType::LowBranch, 6 * TILE_SZ, 0),
            (3, RiverObstacleType::TigerSnake, 7 * TILE_SZ, 1),
            (4, RiverObstacleType::PaddleBoarder, 5 * TILE_SZ, 1),

            (0, RiverObstacleType::Swimmer, 10 * TILE_SZ, 0),
            (1, RiverObstacleType::GiantSpider, 11 * TILE_SZ, 0),
            (2, RiverObstacleType::RiverTuber, 12 * TILE_SZ, 1),
            (3, RiverObstacleType::TreeLog, 13 * TILE_SZ, 0),
            (4, RiverObstacleType::LowBranch, 14 * TILE_SZ, 0),

            (1, RiverObstacleType::TigerSnake, 16 * TILE_SZ, 1),
            (2, RiverObstacleType::PaddleBoarder, 17 * TILE_SZ, 1),
            (3, RiverObstacleType::Swimmer, 18 * TILE_SZ, 0),
            (4, RiverObstacleType::RiverTuber, 19 * TILE_SZ, 1),
        ];

        for (i, (lane, kind, z, spd)) in obstacles.iter().enumerate() {
            if i < MAX_RIVER_OBSTACLES {
                self.river_obstacles[i] = RiverObstacle {
                    active: true,
                    kind: *kind,
                    x: lane_x[*lane],
                    z: *z,
                    y: match *kind {
                        RiverObstacleType::LowBranch => -22, // Hanging overhead!
                        _ => 0,
                    },
                    speed: *spd,
                };
            }
        }

        // Koalas on river gums along banks
        if MAX_RIVER_OBSTACLES > 14 {
            self.river_obstacles[14] = RiverObstacle {
                active: true,
                kind: RiverObstacleType::Koala,
                x: 8 * TILE_SZ + 16,
                z: 8 * TILE_SZ,
                y: -30,
                speed: 0,
            };
            self.river_obstacles[15] = RiverObstacle {
                active: true,
                kind: RiverObstacleType::Koala,
                x: 14 * TILE_SZ + 48,
                z: 14 * TILE_SZ,
                y: -30,
                speed: 0,
            };
        }

        // Fresh yabbies along river lanes
        self.spawn_item(0, CollectibleType::YabbyRation, lane_x[2], 3 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, lane_x[0], 8 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, lane_x[4], 11 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::YabbyRation, lane_x[2], 15 * TILE_SZ, true);
        self.spawn_item(4, CollectibleType::LetterPage, lane_x[1], 20 * TILE_SZ, true);
    }

    fn load_act3(&mut self) {
        // Melbourne City Frogger traffic
        let vehicles_data = [
            // Row 18: Westbound Taxis & Sedans
            (VehicleType::Taxi, 6 * TILE_SZ, 19 * TILE_SZ + 32, -3, 34, (240, 200, 30)),
            (VehicleType::Sedan, 14 * TILE_SZ, 19 * TILE_SZ + 32, -2, 32, (40, 110, 220)),
            (VehicleType::Taxi, 20 * TILE_SZ, 19 * TILE_SZ + 32, -3, 34, (240, 200, 30)),

            // Row 18: Westbound fast lane
            (VehicleType::SportsCar, 10 * TILE_SZ, 18 * TILE_SZ + 32, -4, 30, (230, 45, 45)),
            (VehicleType::Sedan, 18 * TILE_SZ, 18 * TILE_SZ + 32, -2, 32, (180, 180, 190)),

            // Row 13 & 14: Melbourne Tram tracks
            (VehicleType::Tram, 4 * TILE_SZ, 13 * TILE_SZ + 32, 2, 72, (30, 160, 70)),
            (VehicleType::Tram, 16 * TILE_SZ, 14 * TILE_SZ + 32, -2, 72, (30, 160, 70)),

            // Row 7 & 8: Eastbound Highway (Trucks & Sports cars)
            (VehicleType::Truck, 5 * TILE_SZ, 8 * TILE_SZ + 32, 2, 64, (210, 50, 40)),
            (VehicleType::Sedan, 15 * TILE_SZ, 8 * TILE_SZ + 32, 3, 32, (50, 90, 160)),
            (VehicleType::SportsCar, 8 * TILE_SZ, 7 * TILE_SZ + 32, 4, 30, (255, 230, 60)),
            (VehicleType::Truck, 18 * TILE_SZ, 7 * TILE_SZ + 32, 2, 64, (60, 140, 200)),
        ];

        for (i, (kind, x, z, vx, len, col)) in vehicles_data.iter().enumerate() {
            if i < MAX_VEHICLES {
                self.vehicles[i] = CityVehicle {
                    active: true,
                    kind: *kind,
                    x: *x,
                    z: *z,
                    vx: *vx,
                    length: *len,
                    color: *col,
                };
            }
        }

        // City snacks yabbies on medians and sidewalks
        self.spawn_item(0, CollectibleType::YabbyRation, 7 * TILE_SZ, 21 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 12 * TILE_SZ, 16 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 16 * TILE_SZ, 10 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::LetterPage, 12 * TILE_SZ, 3 * TILE_SZ, true);
    }

    fn load_act4(&mut self) {
        // Coastal Beach 3D Platformer
        // Stepped rock platforms & Bouncing parasols
        let platforms = [
            (6 * TILE_SZ, -20, 16 * TILE_SZ, 48, 20, 48, false),
            (8 * TILE_SZ, -2, 14 * TILE_SZ, 36, 16, 36, true), // Bouncing parasol 1!
            (10 * TILE_SZ, -38, 12 * TILE_SZ, 56, 38, 56, false),
            (14 * TILE_SZ, -4, 10 * TILE_SZ, 36, 16, 36, true), // Bouncing parasol 2!
            (16 * TILE_SZ, -58, 8 * TILE_SZ, 60, 58, 60, false),
            (18 * TILE_SZ, -75, 4 * TILE_SZ, 64, 75, 64, false), // Peak dune leading to Pip!
        ];

        for (i, (x, y, z, w, h, d, is_p)) in platforms.iter().enumerate() {
            if i < MAX_BEACH_PLATFORMS {
                self.beach_platforms[i] = BeachPlatform {
                    active: true,
                    x: *x,
                    y: *y,
                    z: *z,
                    w: *w,
                    h: *h,
                    d: *d,
                    is_parasol: *is_p,
                };
            }
        }

        // Beach Crabs patrolling the sand
        self.beach_crabs[0] = BeachCrab {
            active: true,
            x: 6 * TILE_SZ,
            y: 0,
            z: 18 * TILE_SZ,
            min_x: 4 * TILE_SZ,
            max_x: 10 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[1] = BeachCrab {
            active: true,
            x: 12 * TILE_SZ,
            y: 0,
            z: 13 * TILE_SZ,
            min_x: 9 * TILE_SZ,
            max_x: 15 * TILE_SZ,
            vx: -2,
        };
        self.beach_crabs[2] = BeachCrab {
            active: true,
            x: 14 * TILE_SZ,
            y: -58,
            z: 8 * TILE_SZ,
            min_x: 13 * TILE_SZ,
            max_x: 17 * TILE_SZ,
            vx: 1,
        };

        // Golden Star Yabbies on high ledges!
        self.spawn_item(0, CollectibleType::StarYabby, 6 * TILE_SZ + 24, 16 * TILE_SZ + 24, true);
        self.collectibles[0].y = -30;
        self.spawn_item(1, CollectibleType::StarYabby, 10 * TILE_SZ + 28, 12 * TILE_SZ + 28, true);
        self.collectibles[1].y = -48;
        self.spawn_item(2, CollectibleType::StarYabby, 16 * TILE_SZ + 30, 8 * TILE_SZ + 30, true);
        self.collectibles[2].y = -68;
        self.spawn_item(3, CollectibleType::YabbyRation, 4 * TILE_SZ, 19 * TILE_SZ, true);
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
            attack_cooldown: 0,
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

    /// Update entity AI and stage-specific mechanics.
    pub fn update(
        &mut self,
        act: Act,
        player_x: i32,
        _player_y: i32,
        player_z: i32,
        player_crawling: bool,
        player_sneaking: bool,
        player_submerged: bool,
        level: &Level,
    ) {
        self.frame = self.frame.wrapping_add(1);

        match act {
            Act::Act1Sanctuary => {
                self.update_act1(player_x, player_z, player_crawling, player_sneaking, player_submerged, level);
            }
            Act::Act1Boss => {
                self.update_act1_boss(player_x, player_z, player_crawling, level);
            }
            Act::Act2Bushland => {
                self.update_act2();
            }
            Act::Act3City => {
                self.update_act3();
            }
            Act::Act4Ocean => {
                self.update_act4();
            }
        }

        // Update 3D particles across all acts
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

    fn update_act1(
        &mut self,
        player_x: i32,
        player_z: i32,
        player_crawling: bool,
        player_sneaking: bool,
        player_submerged: bool,
        level: &Level,
    ) {
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
                    self.alert_state = AlertState::Caution(300);
                    AudioManager::set_bgm(BgmTrack::Stealth);
                }
            }
        }

        let is_in_alert = matches!(self.alert_state, AlertState::Alert(_));
        let mut alert_triggered = false;

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

            if !player_submerged {
                let dx = (player_x - s.beam_x).abs();
                let dz = (player_z - s.beam_z).abs();
                if dx < 28 && dz < 28 {
                    alert_triggered = true;
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
            d.x += (cos_1_3_12(d.angle) as i32 * 2) >> 12;
            d.z += (sin_1_3_12(d.angle) as i32 * 2) >> 12;

            if !player_submerged && !player_crawling {
                let dx = (player_x - d.x).abs();
                let dz = (player_z - d.z).abs();
                if dx < 36 && dz < 36 {
                    alert_triggered = true;
                }
            }
        }

        // Update Sentries with robust vision & alert chase AI
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

            if s.attack_cooldown > 0 {
                s.attack_cooldown -= 1;
            }

            let mut detected = false;
            if !player_submerged {
                let dx = player_x - s.x;
                let dz = player_z - s.z;
                let dist_sq = dx * dx + dz * dz;

                // Vision distance range: 240 units
                if dist_sq < (240 * 240) {
                    let mid_x = (s.x + player_x) / 2;
                    let mid_z = (s.z + player_z) / 2;
                    let obstructed = level.is_solid_at(mid_x, mid_z, false);

                    if !obstructed {
                        // Close proximity hearing if player is not crawling or sneaking
                        let heard = dist_sq < (52 * 52) && !player_crawling && !player_sneaking;

                        // Forward vector for angle (sin for X, cos for Z)
                        let fwd_x = sin_1_3_12(s.angle) as i32;
                        let fwd_z = cos_1_3_12(s.angle) as i32;
                        let dot = (dx * fwd_x + dz * fwd_z) >> 12;
                        let in_cone = dot > 0 && (dot * dot) >= (dist_sq * 3) / 4;

                        let in_grass = level.is_tall_grass_at(player_x, player_z);
                        let hidden = player_crawling && in_grass;

                        if (heard || in_cone) && !hidden {
                            detected = true;
                        }
                    }
                }
            }

            s.see_player = detected;
            if detected {
                s.state = SentryState::AlertChase;
                alert_triggered = true;
            }

            match s.state {
                SentryState::Stunned => {}
                SentryState::AlertChase => {
                    // Chase player directly
                    let dx = player_x - s.x;
                    let dz = player_z - s.z;

                    if dx.abs() > 4 {
                        s.x += if dx > 0 { 2 } else { -2 };
                    }
                    if dz.abs() > 4 {
                        s.z += if dz > 0 { 2 } else { -2 };
                    }

                    // Aim facing towards player
                    if dx.abs() > dz.abs() {
                        s.angle = if dx > 0 { 64 } else { 192 };
                    } else {
                        s.angle = if dz > 0 { 0 } else { 128 };
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
                            s.wait_timer += 1;
                            if s.wait_timer > 60 {
                                s.wait_timer = 0;
                                s.current_waypoint = (s.current_waypoint + 1) % s.waypoint_count;
                            }
                        } else {
                            if dx.abs() > 2 {
                                s.x += if dx > 0 { 1 } else { -1 };
                                s.angle = if dx > 0 { 64 } else { 192 };
                            }
                            if dz.abs() > 2 {
                                s.z += if dz > 0 { 1 } else { -1 };
                                s.angle = if dz > 0 { 0 } else { 128 };
                            }
                        }
                    }
                }
            }
        }

        if alert_triggered {
            self.trigger_alert();
        }
    }

    fn update_act1_boss(
        &mut self,
        player_x: i32,
        player_z: i32,
        player_crawling: bool,
        level: &Level,
    ) {
        // 1. Update Power Conduits & Shield status
        let mut active_conduits = 0;
        let mut spark_pos = [(0, 0); 3];
        let mut spark_count = 0;

        for (i, c) in self.power_conduits.iter_mut().enumerate() {
            if c.active && !c.destroyed {
                active_conduits += 1;
                if c.spark_timer > 0 {
                    c.spark_timer -= 1;
                }
                // Electrical arcing particles
                if (self.frame.wrapping_add(i as u16 * 17) % 8) == 0 {
                    spark_pos[spark_count] = (c.x, c.z);
                    spark_count += 1;
                }
            }
        }

        for i in 0..spark_count {
            let (sx, sz) = spark_pos[i];
            self.spawn_particle(sx, -16, sz, 0, -2, 0, 18, (80, 210, 255), 2);
        }

        let shields_were_active = self.boss_mech.shield_active;
        self.boss_mech.shield_active = active_conduits > 0;

        // Shield collapse event
        if shields_were_active && !self.boss_mech.shield_active {
            AudioManager::play_hit();
            AudioManager::play_alert();
            self.boss_mech.state = MechState::Venting(120);
            for _ in 0..6 {
                self.spawn_particle(self.boss_mech.x, -28, self.boss_mech.z, 0, -3, 0, 30, (255, 140, 40), 4);
            }
        }

        // 2. Update Searchlight Mech
        if !self.boss_mech.active {
            return;
        }

        if self.boss_mech.hit_timer > 0 {
            self.boss_mech.hit_timer -= 1;
        }

        // Searchlight Sweep
        self.boss_mech.left_light_angle = self.boss_mech.left_light_angle.wrapping_add(1);
        self.boss_mech.right_light_angle = self.boss_mech.right_light_angle.wrapping_sub(1);

        let sweep_l_x = (cos_1_3_12(self.boss_mech.left_light_angle) as i32 * 80) >> 12;
        let sweep_l_z = (sin_1_3_12(self.boss_mech.left_light_angle) as i32 * 60) >> 12;
        self.boss_mech.left_beam_x = self.boss_mech.x - 30 + sweep_l_x;
        self.boss_mech.left_beam_z = self.boss_mech.z + 100 + sweep_l_z;

        let sweep_r_x = (cos_1_3_12(self.boss_mech.right_light_angle) as i32 * 80) >> 12;
        let sweep_r_z = (sin_1_3_12(self.boss_mech.right_light_angle) as i32 * 60) >> 12;
        self.boss_mech.right_beam_x = self.boss_mech.x + 30 + sweep_r_x;
        self.boss_mech.right_beam_z = self.boss_mech.z + 100 + sweep_r_z;

        // Check if player is detected by searchlights
        let in_trench = level.get_cell((player_x / TILE_SZ) as usize, (player_z / TILE_SZ) as usize) == CellType::AirDuct && player_crawling;
        let in_tall_grass = level.is_tall_grass_at(player_x, player_z) && player_crawling;
        let hidden = in_trench || in_tall_grass;

        let d_left = (player_x - self.boss_mech.left_beam_x).abs().max((player_z - self.boss_mech.left_beam_z).abs());
        let d_right = (player_x - self.boss_mech.right_beam_x).abs().max((player_z - self.boss_mech.right_beam_z).abs());
        let player_spotted = !hidden && (d_left < 36 || d_right < 36);

        // Mech Stomp Cooldown
        if self.boss_mech.stomp_cooldown > 0 {
            self.boss_mech.stomp_cooldown -= 1;
        }

        // Mech state machine
        match self.boss_mech.state {
            MechState::Patrolling => {
                // Walk back and forth along north perimeter
                self.boss_mech.x += self.boss_mech.walk_dir * 2;
                if self.boss_mech.x >= 17 * TILE_SZ {
                    self.boss_mech.walk_dir = -1;
                } else if self.boss_mech.x <= 7 * TILE_SZ {
                    self.boss_mech.walk_dir = 1;
                }
                self.boss_mech.leg_anim = ((self.frame / 6) % 4) as u8;

                if player_spotted || (self.boss_mech.stomp_cooldown == 0 && (player_z - self.boss_mech.z).abs() < 180) {
                    AudioManager::play_alert();
                    self.boss_mech.state = MechState::Targeting(40);
                    self.boss_mech.stomp_cooldown = 220;
                }
            }
            MechState::Targeting(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                    // Pivot torso toward player
                    if (player_x - self.boss_mech.x).abs() > 8 {
                        self.boss_mech.x += if player_x > self.boss_mech.x { 1 } else { -1 };
                    }
                } else {
                    // Leap into stomp!
                    self.boss_mech.state = MechState::Stomping(32);
                    self.boss_mech.y = -36;
                }
            }
            MechState::Stomping(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                    if *t == 12 {
                        // Slam down!
                        self.boss_mech.y = 0;
                        self.boss_mech.shockwave_active = true;
                        self.boss_mech.shockwave_radius = 8;
                        AudioManager::play_metal();
                        // Dust burst
                        for i in 0..8 {
                            let ang = (i * 32) as u16;
                            let vx = (cos_1_3_12(ang) as i32 * 3) >> 12;
                            let vz = (sin_1_3_12(ang) as i32 * 3) >> 12;
                            self.spawn_particle(self.boss_mech.x, -2, self.boss_mech.z, vx as i16, -1, vz as i16, 25, (140, 130, 120), 3);
                        }
                    }
                } else {
                    let vent_time = if self.boss_mech.shield_active { 60 } else { 120 };
                    self.boss_mech.state = MechState::Venting(vent_time);
                }
            }
            MechState::Venting(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                    // Puffs of steam and glowing coolant core
                    if (self.frame % 6) == 0 {
                        self.spawn_particle(self.boss_mech.x, -24, self.boss_mech.z - 20, 0, -2, -1, 20, (230, 230, 240), 3);
                    }
                } else {
                    self.boss_mech.state = MechState::Patrolling;
                }
            }
            MechState::Defeated(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                    // Chain explosions
                    if (self.frame % 8) == 0 {
                        let rx = self.boss_mech.x + (((self.frame as i32 * 13) % 40) - 20);
                        let ry = -10 - (((self.frame as i32 * 7) % 30));
                        let rz = self.boss_mech.z + (((self.frame as i32 * 17) % 40) - 20);
                        self.spawn_particle(rx, ry, rz, 0, -3, 0, 30, (255, 120, 30), 4);
                        AudioManager::play_hit();
                    }
                } else {
                    self.boss_mech.active = false;
                }
            }
        }

        // Expand shockwave ring
        if self.boss_mech.shockwave_active {
            self.boss_mech.shockwave_radius += 6;
            if self.boss_mech.shockwave_radius > 200 {
                self.boss_mech.shockwave_active = false;
            }
        }
    }

    fn update_act2(&mut self) {
        self.river_distance += 2;
        // Obstacles stream downriver
        for obs in self.river_obstacles.iter_mut() {
            if !obs.active {
                continue;
            }
            if obs.speed > 0 {
                obs.z += obs.speed;
                if obs.z > 22 * TILE_SZ {
                    obs.z = 2 * TILE_SZ;
                }
            }
        }
    }

    fn update_act3(&mut self) {
        // Cars and trams move across city streets
        for v in self.vehicles.iter_mut() {
            if !v.active {
                continue;
            }
            v.x += v.vx;
            if v.vx > 0 && v.x > 21 * TILE_SZ {
                v.x = 2 * TILE_SZ;
            } else if v.vx < 0 && v.x < 2 * TILE_SZ {
                v.x = 21 * TILE_SZ;
            }
        }
    }

    fn update_act4(&mut self) {
        // Crabs scuttle back and forth along sandy paths
        for crab in self.beach_crabs.iter_mut() {
            if !crab.active {
                continue;
            }
            crab.x += crab.vx;
            if crab.x <= crab.min_x || crab.x >= crab.max_x {
                crab.vx = -crab.vx;
            }
        }
    }
}
