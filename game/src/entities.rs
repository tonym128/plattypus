//! 3D Tactical Entities & Multi-Theme Mechanics:
//! Act 1: Sentries with vision cones, Cypher drones, searchlights, Soliton Radar.
//! Act 2: Yarra River 5-lane obstacles (Trees, branches, snakes, spiders, tubers, paddle boarders, swimmers, koalas).
//! Act 3: Melbourne City Frogger traffic (Taxis, sedans, trams, trucks, sports cars).
//! Act 4: Coastal Beach 3D platformer (Stepped rock ledges, bouncing parasols, beach crabs).

use crate::audio::{AudioManager, BgmTrack};
use crate::level::{Act, CellType, Level, TILE_SZ};
use psx_gte_core::transform::{cos_1_3_12, sin_1_3_12};

pub const MAX_SENTRIES: usize = 6;

// Alert lifecycle, in frames.
pub const ALERT_DURATION: u16 = 600;
pub const CAUTION_DURATION: u16 = 300;
/// How often an ongoing alert may be topped back up to full. Detection runs
/// every frame, so refreshing unconditionally made the alert immortal.
pub const ALERT_REFRESH_COOLDOWN: u8 = 60;

/// Radius of a drone's patrol circle, in world units. The previous value of 2
/// kept a drone inside a two-unit jitter around its spawn point.
pub const DRONE_ORBIT_RADIUS: i32 = 140;
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
    pub investigate_target: (i32, i32),
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
            investigate_target: (0, 0),
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
    /// Centre of the drone's patrol circle. The drone used to be offset from
    /// its own spawn point by a hardcoded 2 units, so it jittered in place
    /// instead of patrolling.
    pub orbit_x: i32,
    pub orbit_z: i32,
    pub orbit_radius: i32,
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
            orbit_x: 0,
            orbit_z: 0,
            orbit_radius: 0,
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

#[derive(Copy, Clone, Debug)]
pub struct JetSkiBoss {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub vx: i32,
    pub health: u8,
    pub max_health: u8,
    pub state_timer: u16,
    pub hit_timer: u8,
    pub is_stalled: bool,
    pub mine_cooldown: u16,
    pub target_lane: usize,
}

impl JetSkiBoss {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            vx: 0,
            health: 3,
            max_health: 3,
            state_timer: 0,
            hit_timer: 0,
            is_stalled: false,
            mine_cooldown: 90,
            target_lane: 2,
        }
    }

    pub fn is_defeated(&self) -> bool {
        self.health == 0 || !self.active
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SniperBoss {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub health: u8,
    pub max_health: u8,
    pub aim_timer: u16,
    pub laser_x: i32,
    pub laser_z: i32,
    pub hit_timer: u8,
    pub perch_index: usize,
    pub is_vulnerable: bool,
}

impl SniperBoss {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: -48,
            z: 0,
            health: 3,
            max_health: 3,
            aim_timer: 0,
            laser_x: 0,
            laser_z: 0,
            hit_timer: 0,
            perch_index: 1,
            is_vulnerable: false,
        }
    }

    pub fn is_defeated(&self) -> bool {
        self.health == 0 || !self.active
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ExcavatorBoss {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub health: u8,
    pub max_health: u8,
    pub claw_angle: u16,
    pub sweep_dir: i16,
    pub state_timer: u16,
    pub hit_timer: u8,
    pub slime_cooldown: u16,
    pub shields_down: bool,
    /// Remaining hits on each of the three engines. Taking one down drops the
    /// machine into a venting window, which is the player's only safe opening.
    pub engine_hp: [u8; EXCAVATOR_ENGINES],
    pub state: ExcavatorState,
}

/// Phase machine for the Act 4-3 climax. The three engines are the fight: each
/// one dropped forces a recovery window, and the machine only dies once all
/// three are gone.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ExcavatorState {
    /// Shields up. Claw sweep and slime mortar only; the hull cannot be hurt.
    Shielded,
    /// An engine was just destroyed. The machine vents for
    /// `EXCAVATOR_VENTING_FRAMES`, during which the hull takes damage.
    Venting(u16),
    /// All engines gone, shields permanently down.
    Exposed,
    Defeated(u16),
}

/// Number of destructible engines on the excavator.
pub const EXCAVATOR_ENGINES: usize = 3;
/// How long the machine vents after an engine is destroyed.
pub const EXCAVATOR_VENTING_FRAMES: u16 = 150;
/// Engine health when the fight starts.
pub const EXCAVATOR_ENGINE_HP: u8 = 2;
/// Striking radius of one engine.
pub const EXCAVATOR_ENGINE_HIT_RADIUS: i32 = 40;
/// How long the death animation plays after the final engine falls.
pub const EXCAVATOR_DEATH_FRAMES: u16 = 120;

/// X offset of each engine relative to the machine's centre, so the player can
/// tell which one they are about to hit.
pub const EXCAVATOR_ENGINE_OFFSETS: [(i32, i32); EXCAVATOR_ENGINES] =
    [(-90, 0), (0, -70), (90, 0)];

impl ExcavatorBoss {
    pub const fn empty() -> Self {
        Self {
            active: false,
            x: 0,
            y: 0,
            z: 0,
            health: 4,
            max_health: 4,
            claw_angle: 0,
            sweep_dir: 1,
            state_timer: 0,
            hit_timer: 0,
            slime_cooldown: 120,
            shields_down: false,
            engine_hp: [EXCAVATOR_ENGINE_HP; EXCAVATOR_ENGINES],
            state: ExcavatorState::Shielded,
        }
    }

    pub fn is_defeated(&self) -> bool {
        self.health == 0 || !self.active
    }

    /// True while the hull is open to damage.
    pub fn is_vulnerable(&self) -> bool {
        self.active
            && !self.is_defeated()
            && matches!(self.state, ExcavatorState::Venting(_) | ExcavatorState::Exposed)
    }

    /// Index of an active, surviving engine within striking distance of a world position, if any.
    pub fn engine_at(&self, wx: i32, wz: i32) -> Option<usize> {
        if !self.active {
            return None;
        }
        EXCAVATOR_ENGINE_OFFSETS
            .iter()
            .enumerate()
            .position(|(i, (dx, dz))| {
                self.engine_hp[i] > 0
                    && (wx - (self.x + dx)).abs() <= EXCAVATOR_ENGINE_HIT_RADIUS
                    && (wz - (self.z + dz)).abs() <= EXCAVATOR_ENGINE_HIT_RADIUS
            })
    }

    /// Apply a strike to an engine, returning true if that engine was destroyed.
    ///
    /// Destroying the last engine leaves the machine permanently exposed; any
    /// other engine forces a venting window.
    pub fn damage_engine(&mut self, index: usize) -> bool {
        if index >= EXCAVATOR_ENGINES || self.engine_hp[index] == 0 {
            return false;
        }
        self.engine_hp[index] -= 1;
        if self.engine_hp[index] > 0 {
            return false;
        }
        let all_down = self.engine_hp.iter().all(|hp| *hp == 0);
        if all_down {
            self.shields_down = true;
            self.state = ExcavatorState::Exposed;
        } else {
            self.state = ExcavatorState::Venting(EXCAVATOR_VENTING_FRAMES);
        }
        self.state_timer = 0;
        true
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CollectibleType {
    YabbyRation,   // Restores health +1 heart
    ChaffBattery,  // Powers electro-bill
    LetterPage,    // Intelligence intel from parents
    BuriedYabby,   // Hidden ration, requires electro pulse
    StarYabby,     // Mario 64 star yabby in Act 4
    CardboardBox,  // "The Bill Box" tactical concealment disguise!
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

impl BeachCrab {
    /// Advance a crab one frame, reversing it at the ends of its patrol.
    ///
    /// Two copies of this existed: one used `.abs()`, which cannot restore a
    /// direction once `vx` reaches zero, and one negated unconditionally, which
    /// flipped a stationary crab back and forth every frame. A crab that
    /// spawned past a bound also reversed on every single frame. Reversing
    /// only on a real sign change fixes all three.
    pub fn patrol_step(&mut self) {
        self.x += self.vx;
        // Reversal needs a real direction to flip. A crab spawned past a bound
        // is simply outside its range, so clamp it back in first and send it the
        // other way, otherwise it walks away from the patrol forever.
        if self.x > self.max_x {
            self.x = self.max_x;
            self.vx = -self.vx.abs().max(1);
        } else if self.x < self.min_x {
            self.x = self.min_x;
            self.vx = self.vx.abs().max(1);
        } else if self.vx > 0 && self.x >= self.max_x {
            self.vx = -self.vx;
        } else if self.vx < 0 && self.x <= self.min_x {
            self.vx = -self.vx;
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
    /// The act currently loaded, so an expiring alert can restore that act's
    /// own music instead of a hardcoded Act 1 track.
    pub act: Act,
    /// Frames until the alert countdown may be refreshed again. Detection runs
    /// every frame, and refreshing every frame meant the alert could never
    /// expire while the player stayed in a beam.
    alert_refresh_cooldown: u8,
    pub sentries: [Sentry; MAX_SENTRIES],
    pub drones: [Drone; MAX_DRONES],
    pub searchlights: [Searchlight; MAX_SEARCHLIGHTS],
    pub boss_mech: SearchlightMech,
    pub power_conduits: [PowerConduit; 3],
    pub boss_jetski: JetSkiBoss,
    pub boss_sniper: SniperBoss,
    pub boss_excavator: ExcavatorBoss,
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
            act: Act::Act1_1Drainage,
            alert_refresh_cooldown: 0,
            sentries: [Sentry::empty(); MAX_SENTRIES],
            drones: [Drone::empty(); MAX_DRONES],
            searchlights: [Searchlight::empty(); MAX_SEARCHLIGHTS],
            boss_mech: SearchlightMech::empty(),
            power_conduits: [PowerConduit::empty(); 3],
            boss_jetski: JetSkiBoss::empty(),
            boss_sniper: SniperBoss::empty(),
            boss_excavator: ExcavatorBoss::empty(),
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

    /// Raise or extend the red alert.
    ///
    /// Detection calls this on every frame the player is spotted, so an
    /// unconditional reset pinned the countdown at full and guards chased
    /// forever. The countdown is now only topped up at most once per
    /// [`ALERT_REFRESH_COOLDOWN`], which keeps a guard on you for a while
    /// after you break line of sight without making the alert immortal.
    pub fn trigger_alert(&mut self) {
        if !matches!(self.alert_state, AlertState::Alert(_)) {
            self.alert_state = AlertState::Alert(ALERT_DURATION);
            self.alert_refresh_cooldown = ALERT_REFRESH_COOLDOWN;
            AudioManager::play_alert();
            AudioManager::set_bgm(BgmTrack::Alert);
        } else if self.alert_refresh_cooldown == 0 {
            if let AlertState::Alert(ref mut timer) = self.alert_state {
                *timer = ALERT_DURATION;
            }
            self.alert_refresh_cooldown = ALERT_REFRESH_COOLDOWN;
        }
    }

    /// The music an act plays when nothing is happening. Alert decay used to
    /// restore a fixed Act 1 track, so expiring an alert in Act 2-2 or 4-2
    /// left the wrong music playing for the rest of the stage.
    fn ambient_bgm(&self) -> BgmTrack {
        match self.act {
            Act::Act2_1Rapids | Act::Act2_2Mangroves | Act::Act2_3JetSkiBoss => BgmTrack::River,
            Act::Act4_1Dunes | Act::Act4_2PierTrench | Act::Act4_3ExcavatorBoss => BgmTrack::Beach,
            _ => BgmTrack::Stealth,
        }
    }

    pub fn load_act(&mut self, act: Act) {
        self.alert_state = AlertState::Sneaking;
        self.act = act;
        self.alert_refresh_cooldown = 0;
        self.sentries = [Sentry::empty(); MAX_SENTRIES];
        self.drones = [Drone::empty(); MAX_DRONES];
        self.searchlights = [Searchlight::empty(); MAX_SEARCHLIGHTS];
        self.boss_mech = SearchlightMech::empty();
        self.power_conduits = [PowerConduit::empty(); 3];
        self.boss_jetski = JetSkiBoss::empty();
        self.boss_sniper = SniperBoss::empty();
        self.boss_excavator = ExcavatorBoss::empty();
        self.collectibles = [Collectible::empty(); MAX_COLLECTIBLES];
        self.particles = [Particle3D::empty(); MAX_PARTICLES];
        self.river_obstacles = [RiverObstacle::empty(); MAX_RIVER_OBSTACLES];
        self.vehicles = [CityVehicle::empty(); MAX_VEHICLES];
        self.beach_platforms = [BeachPlatform::empty(); MAX_BEACH_PLATFORMS];
        self.beach_crabs = [BeachCrab::empty(); MAX_BEACH_CRABS];
        self.river_distance = 0;

        match act {
            Act::Act1_1Drainage => self.load_act1_1(),
            Act::Act1_2Barracks => self.load_act1_2(),
            Act::Act1_3MechBoss => self.load_act1_3(),
            Act::Act2_1Rapids => self.load_act2_1(),
            Act::Act2_2Mangroves => self.load_act2_2(),
            Act::Act2_3JetSkiBoss => self.load_act2_3(),
            Act::Act3_1Highway => self.load_act3_1(),
            Act::Act3_2Laneways => self.load_act3_2(),
            Act::Act3_3SniperBoss => self.load_act3_3(),
            Act::Act4_1Dunes => self.load_act4_1(),
            Act::Act4_2PierTrench => self.load_act4_2(),
            Act::Act4_3ExcavatorBoss => self.load_act4_3(),
            Act::VrSneaking => self.load_vr_sneaking(),
            Act::VrCqc => self.load_vr_cqc(),
            Act::VrSonar => self.load_vr_sonar(),
            Act::VrSpeed => self.load_vr_speed(),
        }
    }

    // -------------------------------------------------------------------------
    // ACT 1-1: DRAINAGE OUTFLOW
    // -------------------------------------------------------------------------
    fn load_act1_1(&mut self) {
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

        self.spawn_item(0, CollectibleType::YabbyRation, 4 * TILE_SZ, 7 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::LetterPage, 11 * TILE_SZ, 3 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 14 * TILE_SZ, 18 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::BuriedYabby, 3 * TILE_SZ, 12 * TILE_SZ, false);
        self.spawn_item(4, CollectibleType::CardboardBox, 10 * TILE_SZ, 8 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 1-2: RESEARCH BARRACKS & LASER GRID
    // -------------------------------------------------------------------------
    fn load_act1_2(&mut self) {
        self.spawn_sentry(0, 3 * TILE_SZ, 6 * TILE_SZ, &[
            (3 * TILE_SZ, 6 * TILE_SZ),
            (3 * TILE_SZ, 16 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 11 * TILE_SZ, 5 * TILE_SZ, &[
            (11 * TILE_SZ, 5 * TILE_SZ),
            (11 * TILE_SZ, 17 * TILE_SZ),
        ]);
        self.spawn_sentry(2, 19 * TILE_SZ, 6 * TILE_SZ, &[
            (19 * TILE_SZ, 6 * TILE_SZ),
            (19 * TILE_SZ, 16 * TILE_SZ),
        ]);

        self.drones[0] = Drone {
            active: true,
            x: 7 * TILE_SZ,
            y: -36,
            z: 10 * TILE_SZ,
            angle: 0,
            stun_timer: 0,
            orbit_x: 7 * TILE_SZ,
            orbit_z: 10 * TILE_SZ,
            orbit_radius: DRONE_ORBIT_RADIUS,
        };
        self.drones[1] = Drone {
            active: true,
            x: 15 * TILE_SZ,
            y: -36,
            z: 10 * TILE_SZ,
            angle: 128,
            stun_timer: 0,
            orbit_x: 15 * TILE_SZ,
            orbit_z: 10 * TILE_SZ,
            orbit_radius: DRONE_ORBIT_RADIUS,
        };

        self.spawn_item(0, CollectibleType::CardboardBox, 3 * TILE_SZ, 19 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 3 * TILE_SZ, 5 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 19 * TILE_SZ, 5 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::LetterPage, 19 * TILE_SZ, 19 * TILE_SZ, true);
        self.spawn_item(4, CollectibleType::BuriedYabby, 11 * TILE_SZ, 14 * TILE_SZ, false);
    }

    // -------------------------------------------------------------------------
    // ACT 1-3: PERIMETER WALL (SEARCHLIGHT MECH BOSS)
    // -------------------------------------------------------------------------
    fn load_act1_3(&mut self) {
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

        self.power_conduits[0] = PowerConduit {
            active: true,
            destroyed: false,
            x: 5 * TILE_SZ + 32,
            z: 9 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };
        self.power_conduits[1] = PowerConduit {
            active: true,
            destroyed: false,
            x: 19 * TILE_SZ + 32,
            z: 9 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };
        self.power_conduits[2] = PowerConduit {
            active: true,
            destroyed: false,
            x: 12 * TILE_SZ + 32,
            z: 17 * TILE_SZ + 32,
            health: 3,
            spark_timer: 0,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, 3 * TILE_SZ + 32, 9 * TILE_SZ + 32, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 20 * TILE_SZ + 32, 9 * TILE_SZ + 32, true);
    }

    // -------------------------------------------------------------------------
    // ACT 2-1: UPPER GORGE RAPIDS (5-LANE RIVER RUNNER)
    // -------------------------------------------------------------------------
    fn load_act2_1(&mut self) {
        let lane_x = [
            9 * TILE_SZ + 32,
            10 * TILE_SZ + 32,
            11 * TILE_SZ + 32,
            12 * TILE_SZ + 32,
            13 * TILE_SZ + 32,
        ];

        let obstacles = [
            (0, RiverObstacleType::TreeLog, 18 * TILE_SZ, 0),
            (1, RiverObstacleType::RiverTuber, 17 * TILE_SZ, 1),
            (2, RiverObstacleType::LowBranch, 16 * TILE_SZ, 0),
            (3, RiverObstacleType::TigerSnake, 15 * TILE_SZ, 1),
            (4, RiverObstacleType::PaddleBoarder, 14 * TILE_SZ, 1),

            (0, RiverObstacleType::Swimmer, 12 * TILE_SZ, 0),
            (1, RiverObstacleType::GiantSpider, 11 * TILE_SZ, 0),
            (2, RiverObstacleType::RiverTuber, 10 * TILE_SZ, 1),
            (3, RiverObstacleType::TreeLog, 9 * TILE_SZ, 0),
            (4, RiverObstacleType::LowBranch, 8 * TILE_SZ, 0),

            (1, RiverObstacleType::TigerSnake, 6 * TILE_SZ, 1),
            (2, RiverObstacleType::PaddleBoarder, 5 * TILE_SZ, 1),
            (3, RiverObstacleType::Swimmer, 4 * TILE_SZ, 0),
            (4, RiverObstacleType::RiverTuber, 3 * TILE_SZ, 1),
        ];

        for (i, (lane, kind, z, spd)) in obstacles.iter().enumerate() {
            if i < MAX_RIVER_OBSTACLES {
                self.river_obstacles[i] = RiverObstacle {
                    active: true,
                    kind: *kind,
                    x: lane_x[*lane],
                    z: *z,
                    y: match *kind {
                        RiverObstacleType::LowBranch => -22,
                        _ => 0,
                    },
                    speed: *spd,
                };
            }
        }

        self.spawn_item(0, CollectibleType::YabbyRation, lane_x[2], 19 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, lane_x[0], 13 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, lane_x[4], 10 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::YabbyRation, lane_x[2], 7 * TILE_SZ, true);
        self.spawn_item(4, CollectibleType::LetterPage, lane_x[1], 3 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 2-2: DANDENONG MURKY MANGROVES & CAVERN MAZE
    // -------------------------------------------------------------------------
    fn load_act2_2(&mut self) {
        // Huntsman Spiders patrolling murky channels
        self.beach_crabs[0] = BeachCrab {
            active: true,
            x: 4 * TILE_SZ,
            y: 0,
            z: 7 * TILE_SZ,
            min_x: 2 * TILE_SZ,
            max_x: 6 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[1] = BeachCrab {
            active: true,
            x: 10 * TILE_SZ,
            y: 0,
            z: 12 * TILE_SZ,
            min_x: 8 * TILE_SZ,
            max_x: 12 * TILE_SZ,
            vx: -2,
        };
        self.beach_crabs[2] = BeachCrab {
            active: true,
            x: 15 * TILE_SZ,
            y: 0,
            z: 8 * TILE_SZ,
            min_x: 13 * TILE_SZ,
            max_x: 17 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[3] = BeachCrab {
            active: true,
            x: 8 * TILE_SZ,
            y: 0,
            z: 18 * TILE_SZ,
            min_x: 6 * TILE_SZ,
            max_x: 11 * TILE_SZ,
            vx: -2,
        };

        // River ranger guard at dock
        self.spawn_sentry(0, 19 * TILE_SZ, 14 * TILE_SZ, &[
            (19 * TILE_SZ, 14 * TILE_SZ),
            (19 * TILE_SZ, 19 * TILE_SZ),
        ]);

        self.spawn_item(0, CollectibleType::BuriedYabby, 4 * TILE_SZ, 12 * TILE_SZ, false);
        self.spawn_item(1, CollectibleType::BuriedYabby, 10 * TILE_SZ, 4 * TILE_SZ, false);
        self.spawn_item(2, CollectibleType::BuriedYabby, 16 * TILE_SZ, 17 * TILE_SZ, false);
        self.spawn_item(3, CollectibleType::YabbyRation, 12 * TILE_SZ, 10 * TILE_SZ, true);
        self.spawn_item(4, CollectibleType::LetterPage, 19 * TILE_SZ, 4 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 2-3: RIVER RAPIDS PURSUIT (PARK RANGER JET SKI BOSS)
    // -------------------------------------------------------------------------
    fn load_act2_3(&mut self) {
        self.boss_jetski = JetSkiBoss {
            active: true,
            x: 11 * TILE_SZ + 32,
            y: 0,
            z: 8 * TILE_SZ,
            vx: 2,
            health: 3,
            max_health: 3,
            state_timer: 0,
            hit_timer: 0,
            is_stalled: false,
            mine_cooldown: 90,
            target_lane: 2,
        };

        let lane_x = [
            9 * TILE_SZ + 32,
            10 * TILE_SZ + 32,
            11 * TILE_SZ + 32,
            12 * TILE_SZ + 32,
            13 * TILE_SZ + 32,
        ];
        self.river_obstacles[0] = RiverObstacle {
            active: true,
            kind: RiverObstacleType::TreeLog,
            x: lane_x[1],
            z: 14 * TILE_SZ,
            y: 0,
            speed: 0,
        };
        self.river_obstacles[1] = RiverObstacle {
            active: true,
            kind: RiverObstacleType::TreeLog,
            x: lane_x[3],
            z: 11 * TILE_SZ,
            y: 0,
            speed: 0,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, lane_x[0], 16 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, lane_x[4], 12 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 3-1: MELBOURNE DOWNTOWN (CITY FROGGER)
    // -------------------------------------------------------------------------
    fn load_act3_1(&mut self) {
        let vehicles_data = [
            (VehicleType::Taxi, 6 * TILE_SZ, 19 * TILE_SZ + 32, -3, 34, (240, 200, 30)),
            (VehicleType::Sedan, 14 * TILE_SZ, 19 * TILE_SZ + 32, -2, 32, (40, 110, 220)),
            (VehicleType::Taxi, 20 * TILE_SZ, 19 * TILE_SZ + 32, -3, 34, (240, 200, 30)),
            (VehicleType::SportsCar, 10 * TILE_SZ, 18 * TILE_SZ + 32, -4, 30, (230, 45, 45)),
            (VehicleType::Sedan, 18 * TILE_SZ, 18 * TILE_SZ + 32, -2, 32, (180, 180, 190)),
            (VehicleType::Tram, 4 * TILE_SZ, 13 * TILE_SZ + 32, 2, 72, (30, 160, 70)),
            (VehicleType::Tram, 16 * TILE_SZ, 14 * TILE_SZ + 32, -2, 72, (30, 160, 70)),
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

        self.spawn_item(0, CollectibleType::YabbyRation, 7 * TILE_SZ, 21 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 12 * TILE_SZ, 16 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 16 * TILE_SZ, 10 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::LetterPage, 12 * TILE_SZ, 3 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 3-2: FLINDERS STREET LANEWAYS & ROOFTOPS
    // -------------------------------------------------------------------------
    fn load_act3_2(&mut self) {
        self.spawn_sentry(0, 4 * TILE_SZ, 5 * TILE_SZ, &[
            (4 * TILE_SZ, 5 * TILE_SZ),
            (4 * TILE_SZ, 17 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 10 * TILE_SZ, 6 * TILE_SZ, &[
            (10 * TILE_SZ, 6 * TILE_SZ),
            (10 * TILE_SZ, 16 * TILE_SZ),
        ]);
        self.spawn_sentry(2, 15 * TILE_SZ, 4 * TILE_SZ, &[
            (15 * TILE_SZ, 4 * TILE_SZ),
            (15 * TILE_SZ, 18 * TILE_SZ),
        ]);

        self.searchlights[0] = Searchlight {
            active: true,
            base_x: 12 * TILE_SZ,
            base_z: 10 * TILE_SZ,
            beam_x: 12 * TILE_SZ,
            beam_z: 10 * TILE_SZ,
            sweep_angle: 64,
            radius: 50,
        };

        self.drones[0] = Drone {
            active: true,
            x: 18 * TILE_SZ,
            y: -36,
            z: 10 * TILE_SZ,
            angle: 0,
            stun_timer: 0,
            orbit_x: 18 * TILE_SZ,
            orbit_z: 10 * TILE_SZ,
            orbit_radius: DRONE_ORBIT_RADIUS,
        };

        self.spawn_item(0, CollectibleType::CardboardBox, 4 * TILE_SZ, 19 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 4 * TILE_SZ, 4 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::YabbyRation, 10 * TILE_SZ, 18 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::LetterPage, 19 * TILE_SZ, 10 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 3-3: ANTENNA TOWER & SNIPER KOOKABURRA BOSS
    // -------------------------------------------------------------------------
    fn load_act3_3(&mut self) {
        self.boss_sniper = SniperBoss {
            active: true,
            x: 12 * TILE_SZ + 32,
            y: -48,
            z: 5 * TILE_SZ + 32,
            health: 3,
            max_health: 3,
            aim_timer: 0,
            laser_x: 12 * TILE_SZ,
            laser_z: 16 * TILE_SZ,
            hit_timer: 0,
            perch_index: 1,
            is_vulnerable: false,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, 7 * TILE_SZ, 10 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 16 * TILE_SZ, 10 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 4-1: COASTAL DUNES & SURF (3D PLATFORMER)
    // -------------------------------------------------------------------------
    fn load_act4_1(&mut self) {
        let platforms = [
            (6 * TILE_SZ, -20, 16 * TILE_SZ, 48, 20, 48, false),
            (8 * TILE_SZ, -2, 14 * TILE_SZ, 36, 16, 36, true),
            (10 * TILE_SZ, -38, 12 * TILE_SZ, 56, 38, 56, false),
            (14 * TILE_SZ, -4, 10 * TILE_SZ, 36, 16, 36, true),
            (16 * TILE_SZ, -58, 8 * TILE_SZ, 60, 58, 60, false),
            (18 * TILE_SZ, -75, 4 * TILE_SZ, 64, 75, 64, false),
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

        self.spawn_item(0, CollectibleType::StarYabby, 6 * TILE_SZ + 24, 16 * TILE_SZ + 24, true);
        self.collectibles[0].y = -30;
        self.spawn_item(1, CollectibleType::StarYabby, 10 * TILE_SZ + 28, 12 * TILE_SZ + 28, true);
        self.collectibles[1].y = -48;
        self.spawn_item(2, CollectibleType::StarYabby, 16 * TILE_SZ + 30, 8 * TILE_SZ + 30, true);
        self.collectibles[2].y = -68;
        self.spawn_item(3, CollectibleType::YabbyRation, 4 * TILE_SZ, 19 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 4-2: PIER UNDERSTRUCTURE & SHARK TRENCH
    // -------------------------------------------------------------------------
    fn load_act4_2(&mut self) {
        // Sharks circling water channels
        self.beach_crabs[0] = BeachCrab {
            active: true,
            x: 7 * TILE_SZ,
            y: 0,
            z: 6 * TILE_SZ,
            min_x: 6 * TILE_SZ,
            max_x: 9 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[1] = BeachCrab {
            active: true,
            x: 12 * TILE_SZ,
            y: 0,
            z: 11 * TILE_SZ,
            min_x: 11 * TILE_SZ,
            max_x: 14 * TILE_SZ,
            vx: -2,
        };
        self.beach_crabs[2] = BeachCrab {
            active: true,
            x: 7 * TILE_SZ,
            y: 0,
            z: 16 * TILE_SZ,
            min_x: 6 * TILE_SZ,
            max_x: 9 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[3] = BeachCrab {
            active: true,
            x: 17 * TILE_SZ,
            y: 0,
            z: 8 * TILE_SZ,
            min_x: 16 * TILE_SZ,
            max_x: 20 * TILE_SZ,
            vx: -2,
        };

        // Rest platforms
        self.beach_platforms[0] = BeachPlatform {
            active: true,
            x: 7 * TILE_SZ,
            y: 0,
            z: 14 * TILE_SZ,
            w: 48,
            h: 8,
            d: 48,
            is_parasol: false,
        };
        self.beach_platforms[1] = BeachPlatform {
            active: true,
            x: 12 * TILE_SZ,
            y: 0,
            z: 8 * TILE_SZ,
            w: 48,
            h: 8,
            d: 48,
            is_parasol: false,
        };

        self.spawn_item(0, CollectibleType::StarYabby, 3 * TILE_SZ, 10 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::StarYabby, 12 * TILE_SZ, 8 * TILE_SZ, true);
        self.spawn_item(2, CollectibleType::StarYabby, 17 * TILE_SZ, 12 * TILE_SZ, true);
        self.spawn_item(3, CollectibleType::LetterPage, 18 * TILE_SZ, 4 * TILE_SZ, true);
    }

    // -------------------------------------------------------------------------
    // ACT 4-3: BURROW DEFENSE & DR. CANE TOAD'S EXCAVATOR (FINAL CLIMAX)
    // -------------------------------------------------------------------------
    fn load_act4_3(&mut self) {
        self.boss_excavator = ExcavatorBoss {
            active: true,
            x: 12 * TILE_SZ + 32,
            y: 0,
            z: 7 * TILE_SZ + 32,
            health: 4,
            max_health: 4,
            claw_angle: 0,
            sweep_dir: 1,
            state_timer: 0,
            hit_timer: 0,
            slime_cooldown: 120,
            shields_down: false,
            engine_hp: [EXCAVATOR_ENGINE_HP; EXCAVATOR_ENGINES],
            state: ExcavatorState::Shielded,
        };

        self.spawn_item(0, CollectibleType::YabbyRation, 4 * TILE_SZ, 15 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 19 * TILE_SZ, 15 * TILE_SZ, true);
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
            investigate_target: (0, 0),
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
        player_in_box: bool,
        player_moving: bool,
        noise_radius: i32,
        level: &Level,
    ) {
        self.frame = self.frame.wrapping_add(1);

        match act {
            Act::Act1_1Drainage
            | Act::Act1_2Barracks
            | Act::Act2_2Mangroves
            | Act::Act3_2Laneways
            | Act::Act4_2PierTrench
            | Act::VrSneaking
            | Act::VrCqc
            | Act::VrSonar
            | Act::VrSpeed => {
                self.update_act1(
                    player_x,
                    player_z,
                    player_crawling,
                    player_sneaking,
                    player_submerged,
                    player_in_box,
                    player_moving,
                    noise_radius,
                    level,
                );
                // Also update any water/crab hazards present in 2-2 and 4-2
                if act == Act::Act2_2Mangroves || act == Act::Act4_2PierTrench {
                    for crab in self.beach_crabs.iter_mut() {
                        if !crab.active {
                            continue;
                        }
                        crab.patrol_step();
                    }
                }
            }
            Act::Act1_3MechBoss => {
                self.update_act1_boss(player_x, player_z, player_crawling, level);
            }
            Act::Act2_1Rapids => {
                self.update_act2();
            }
            Act::Act2_3JetSkiBoss => {
                self.update_act2_3_boss();
            }
            Act::Act3_1Highway => {
                self.update_act3();
            }
            Act::Act3_3SniperBoss => {
                self.update_act3_3_boss(player_x, player_z, player_crawling, level);
            }
            Act::Act4_1Dunes => {
                self.update_act4();
            }
            Act::Act4_3ExcavatorBoss => {
                self.update_act4_3_boss(player_x, player_z);
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
        player_in_box: bool,
        player_moving: bool,
        noise_radius: i32,
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
                    self.alert_state = AlertState::Caution(CAUTION_DURATION);
                    AudioManager::set_bgm(self.ambient_bgm());
                }
            }
        }
        if self.alert_refresh_cooldown > 0 {
            self.alert_refresh_cooldown -= 1;
        }

        let is_in_alert = matches!(self.alert_state, AlertState::Alert(_));
        let mut alert_triggered = false;

        // Acoustic Surface Footstep Hearing: Check if unalerted sentries hear footstep noise
        if noise_radius > 0 && !player_submerged {
            let nr_sq = noise_radius * noise_radius;
            for s in self.sentries.iter_mut() {
                if s.active && s.stun_timer == 0 && s.state != SentryState::AlertChase {
                    let dx = player_x - s.x;
                    let dz = player_z - s.z;
                    let d_sq = dx * dx + dz * dz;
                    if d_sq <= nr_sq {
                        // Sentry hears footstep noise! Becomes suspicious and turns to investigate
                        s.state = SentryState::Investigating;
                        s.investigate_target = (player_x, player_z);
                        s.wait_timer = 0;
                        if dx.abs() > dz.abs() {
                            s.angle = if dx > 0 { 64 } else { 192 };
                        } else {
                            s.angle = if dz > 0 { 0 } else { 128 };
                        }
                    }
                }
            }
        }

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
                // A motionless cardboard box is not an immediate alarm under a searchlight beam,
                // but moving under a searchlight is detected!
                let box_safe = player_in_box && !player_moving;
                if dx < 28 && dz < 28 && !box_safe {
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
            // 256 units of angle per revolution; one step is roughly 1.4
            // degrees, so a full circuit takes about four seconds.
            d.angle = d.angle.wrapping_add(1);
            d.x = d.orbit_x + ((cos_1_3_12(d.angle) as i32 * d.orbit_radius) >> 12);
            d.z = d.orbit_z + ((sin_1_3_12(d.angle) as i32 * d.orbit_radius) >> 12);

            if !player_submerged && !player_crawling {
                let dx = (player_x - d.x).abs();
                let dz = (player_z - d.z).abs();
                let box_safe = player_in_box && !player_moving;
                if dx < 36 && dz < 36 && !box_safe {
                    alert_triggered = true;
                }
            }
        }

        // Update Sentries with robust vision, investigating AI, & alert chase
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
                    let obstructed = !level.has_line_of_sight(s.x, s.z, player_x, player_z);

                    if !obstructed {
                        // Close proximity hearing if player is running (and not inside a motionless box)
                        let heard = dist_sq < (52 * 52) && !player_crawling && !player_sneaking && !(player_in_box && !player_moving);

                        // Forward vector for angle (sin for X, cos for Z)
                        let fwd_x = sin_1_3_12(s.angle) as i32;
                        let fwd_z = cos_1_3_12(s.angle) as i32;
                        let dot = (dx * fwd_x + dz * fwd_z) >> 12;
                        let in_cone = dot > 0 && (dot * dot) >= (dist_sq * 3) / 4;

                        let in_grass = level.is_tall_grass_at(player_x, player_z);
                        let hidden = player_crawling && in_grass;

                        // Cardboard Box Concealment:
                        // Motionless box = ignored ("Huh? Just a box...").
                        // Moving in vision cone = immediately triggers suspicion / alert!
                        let box_ignored = player_in_box && !player_moving;

                        if (heard || in_cone) && !hidden && !box_ignored {
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

                    // `is_in_alert` was sampled before the sentry loop, but
                    // the global alert is only raised after it, so a sentry
                    // that acquired the player this frame was demoted straight
                    // back to patrolling and the chase never started. The
                    // per-frame flag covers the frame the contact is made.
                    if !is_in_alert && !alert_triggered {
                        s.state = SentryState::Patrolling;
                    }
                }
                SentryState::Investigating => {
                    // Walk over to the noise location with '?' overhead
                    let target = s.investigate_target;
                    let dx = target.0 - s.x;
                    let dz = target.1 - s.z;

                    if dx.abs() < 8 && dz.abs() < 8 {
                        // Arrived at sound location; pause and look around
                        s.wait_timer += 1;
                        if s.wait_timer % 30 == 0 {
                            s.angle = s.angle.wrapping_add(64);
                        }
                        if s.wait_timer > 90 {
                            // Nobody here! Return to patrol
                            s.wait_timer = 0;
                            s.state = SentryState::Patrolling;
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
                SentryState::Patrolling => {
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
                // `panic = "abort"` makes an out-of-bounds index an
                // unrecoverable lockup, so the write is bounded rather than
                // relying on the array happening to be the same length.
                if (self.frame.wrapping_add(i as u16 * 17) % 8) == 0 && spark_count < spark_pos.len() {
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

        // Shield collapse event. Gated on the mech still being alive and not
        // already recovering: the transition used to overwrite `state`
        // unconditionally, so dropping the last conduit mid-stomp teleported the
        // mech to Venting, and a conduit destroyed after death still played the
        // hit and let the player farm the destroy score during the death
        // animation.
        if shields_were_active
            && !self.boss_mech.shield_active
            && self.boss_mech.active
            && !matches!(self.boss_mech.state, MechState::Venting(_) | MechState::Defeated(_))
        {
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
        // Obstacles stream downriver (flowing North toward exit at z=2)
        for obs in self.river_obstacles.iter_mut() {
            if !obs.active {
                continue;
            }
            if obs.speed > 0 {
                obs.z -= obs.speed;
                if obs.z < 2 * TILE_SZ {
                    obs.z = 21 * TILE_SZ;
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
            crab.patrol_step();
        }
    }

    fn update_act2_3_boss(&mut self) {
        // The river keeps running after the jetski dies. Returning early here
        // skipped `update_act2` below, freezing every log and mine for the rest
        // of the stage while the player was usually still running for the exit.
        if !self.boss_jetski.active || self.boss_jetski.is_defeated() {
            self.update_act2();
            return;
        }
        if self.boss_jetski.hit_timer > 0 {
            self.boss_jetski.hit_timer -= 1;
        }
        self.boss_jetski.state_timer = self.boss_jetski.state_timer.wrapping_add(1);

        if !self.boss_jetski.is_stalled {
            self.boss_jetski.x += self.boss_jetski.vx * 2;
            if self.boss_jetski.x >= 13 * TILE_SZ {
                self.boss_jetski.vx = -1;
            } else if self.boss_jetski.x <= 9 * TILE_SZ {
                self.boss_jetski.vx = 1;
            }

            // Drop floating barrel mines
            if self.boss_jetski.mine_cooldown > 0 {
                self.boss_jetski.mine_cooldown -= 1;
            } else {
                self.boss_jetski.mine_cooldown = 100;
                for obs in self.river_obstacles.iter_mut() {
                    if !obs.active {
                        *obs = RiverObstacle {
                            active: true,
                            kind: RiverObstacleType::RiverTuber,
                            x: self.boss_jetski.x,
                            z: self.boss_jetski.z + 40,
                            y: 0,
                            speed: 2,
                        };
                        break;
                    }
                }
            }

            // Engine overheat stall every 240 frames
            if self.boss_jetski.state_timer % 240 == 0 {
                self.boss_jetski.is_stalled = true;
                AudioManager::play_metal();
            }
        } else {
            // Smoke particles when stalled
            if (self.frame % 5) == 0 {
                self.spawn_particle(self.boss_jetski.x, -16, self.boss_jetski.z, 0, -2, 0, 20, (180, 180, 190), 3);
            }
            if self.boss_jetski.state_timer % 90 == 0 {
                self.boss_jetski.is_stalled = false;
            }
        }

        self.update_act2();
    }

    fn update_act3_3_boss(
        &mut self,
        player_x: i32,
        player_z: i32,
        player_crawling: bool,
        level: &Level,
    ) {
        if !self.boss_sniper.active || self.boss_sniper.is_defeated() {
            return;
        }
        if self.boss_sniper.hit_timer > 0 {
            self.boss_sniper.hit_timer -= 1;
        }

        let perches = [
            (4 * TILE_SZ + 32, 8 * TILE_SZ),
            (12 * TILE_SZ + 32, 5 * TILE_SZ),
            (19 * TILE_SZ + 32, 8 * TILE_SZ),
        ];
        let (px, pz) = perches[self.boss_sniper.perch_index % 3];
        self.boss_sniper.x = px;
        self.boss_sniper.z = pz;

        if !self.boss_sniper.is_vulnerable {
            self.boss_sniper.aim_timer = self.boss_sniper.aim_timer.saturating_add(1);
            self.boss_sniper.laser_x += (player_x - self.boss_sniper.laser_x) / 6;
            self.boss_sniper.laser_z += (player_z - self.boss_sniper.laser_z) / 6;

            if self.boss_sniper.aim_timer >= 110 {
                AudioManager::play_hit();
                let in_vent = level.get_cell((player_x / TILE_SZ) as usize, (player_z / TILE_SZ) as usize) == CellType::AirDuct && player_crawling;
                let laser_dist = (player_x - self.boss_sniper.laser_x).abs().max((player_z - self.boss_sniper.laser_z).abs());
                if laser_dist < 40 && !in_vent {
                    self.spawn_particle(player_x, -16, player_z, 0, -2, 0, 20, (255, 60, 60), 4);
                }
                self.boss_sniper.is_vulnerable = true;
                self.boss_sniper.aim_timer = 0;
            }
        } else {
            self.boss_sniper.aim_timer = self.boss_sniper.aim_timer.saturating_add(1);
            if (self.frame % 6) == 0 {
                self.spawn_particle(self.boss_sniper.x, -52, self.boss_sniper.z, 0, -2, 0, 15, (255, 230, 80), 2);
            }
            if self.boss_sniper.aim_timer >= 120 {
                self.boss_sniper.is_vulnerable = false;
                self.boss_sniper.aim_timer = 0;
                self.boss_sniper.perch_index = (self.boss_sniper.perch_index + 1) % 3;
                AudioManager::play_jump();
            }
        }
    }

    fn update_act4_3_boss(&mut self, player_x: i32, _player_z: i32) {
        if !self.boss_excavator.active || self.boss_excavator.is_defeated() {
            return;
        }
        if self.boss_excavator.hit_timer > 0 {
            self.boss_excavator.hit_timer -= 1;
        }
        let exc = &mut self.boss_excavator;
        if exc.hit_timer > 0 {
            exc.hit_timer -= 1;
        }
        if exc.is_defeated() {
            // Burn out the death timer and stop acting.
            if let ExcavatorState::Defeated(ref mut t) = exc.state {
                *t = t.saturating_sub(1);
                if *t == 0 {
                    exc.active = false;
                }
            } else {
                exc.state = ExcavatorState::Defeated(EXCAVATOR_DEATH_FRAMES);
            }
            return;
        }

        // Phase timer. `state_timer` was previously incremented and never read;
        // it now measures how long the current phase has run, which the renderer
        // uses for the venting flash.
        exc.state_timer = exc.state_timer.wrapping_add(1);

        // Phase transitions.
        match exc.state {
            ExcavatorState::Venting(ref mut t) => {
                if *t > 0 {
                    *t -= 1;
                    if *t == 0 {
                        // Shields back up. The machine is never permanently open
                        // unless every engine is gone.
                        exc.state = if exc.shields_down {
                            ExcavatorState::Exposed
                        } else {
                            ExcavatorState::Shielded
                        };
                        exc.state_timer = 0;
                    }
                }
            }
            _ => {}
        }

        let venting = exc.is_vulnerable();

        // Sweeping shovel claw. It stops while the machine vents, which is the
        // window the player is meant to use.
        if !venting {
            exc.claw_angle = exc.claw_angle.wrapping_add((exc.sweep_dir * 3) as u16);
            if exc.claw_angle > 180 {
                exc.sweep_dir = -1;
            } else if exc.claw_angle < 40 {
                exc.sweep_dir = 1;
            }
        }

        // Slime mortar. Suppressed while venting so the recovery window is
        // actually safe rather than merely a damage opening.
        if venting {
            exc.slime_cooldown = EXCAVATOR_VENTING_FRAMES.min(90);
        } else if exc.slime_cooldown > 0 {
            exc.slime_cooldown -= 1;
        } else {
            let target_z = exc.z + 120;
            exc.slime_cooldown = 110;
            AudioManager::play_swoosh();
            self.spawn_particle(player_x, -10, target_z, 0, 1, 0, 30, (80, 220, 40), 4);
        }
    }

    // -------------------------------------------------------------------------
    // VR TRAINING SIMULATOR LOADERS
    // -------------------------------------------------------------------------
    fn load_vr_sneaking(&mut self) {
        self.spawn_sentry(0, 6 * TILE_SZ, 9 * TILE_SZ, &[
            (6 * TILE_SZ, 9 * TILE_SZ),
            (18 * TILE_SZ, 9 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 18 * TILE_SZ, 15 * TILE_SZ, &[
            (18 * TILE_SZ, 15 * TILE_SZ),
            (6 * TILE_SZ, 15 * TILE_SZ),
        ]);

        self.spawn_item(0, CollectibleType::StarYabby, 12 * TILE_SZ, 4 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 12 * TILE_SZ, 12 * TILE_SZ, true);
    }

    fn load_vr_cqc(&mut self) {
        self.spawn_sentry(0, 7 * TILE_SZ, 10 * TILE_SZ, &[
            (7 * TILE_SZ, 10 * TILE_SZ),
            (7 * TILE_SZ, 9 * TILE_SZ),
        ]);
        self.spawn_sentry(1, 12 * TILE_SZ, 10 * TILE_SZ, &[
            (12 * TILE_SZ, 10 * TILE_SZ),
            (12 * TILE_SZ, 9 * TILE_SZ),
        ]);
        self.spawn_sentry(2, 17 * TILE_SZ, 10 * TILE_SZ, &[
            (17 * TILE_SZ, 10 * TILE_SZ),
            (17 * TILE_SZ, 9 * TILE_SZ),
        ]);

        self.spawn_item(0, CollectibleType::CardboardBox, 12 * TILE_SZ, 19 * TILE_SZ, true);
    }

    fn load_vr_sonar(&mut self) {
        self.spawn_item(0, CollectibleType::BuriedYabby, 4 * TILE_SZ, 10 * TILE_SZ, false);
        self.spawn_item(1, CollectibleType::BuriedYabby, 16 * TILE_SZ, 10 * TILE_SZ, false);
        self.spawn_item(2, CollectibleType::BuriedYabby, 8 * TILE_SZ, 18 * TILE_SZ, false);
        self.spawn_item(3, CollectibleType::StarYabby, 18 * TILE_SZ, 5 * TILE_SZ, false);
    }

    fn load_vr_speed(&mut self) {
        self.beach_crabs[0] = BeachCrab {
            active: true,
            x: 10 * TILE_SZ,
            y: 0,
            z: 14 * TILE_SZ,
            min_x: 9 * TILE_SZ,
            max_x: 15 * TILE_SZ,
            vx: 2,
        };
        self.beach_crabs[1] = BeachCrab {
            active: true,
            x: 14 * TILE_SZ,
            y: 0,
            z: 8 * TILE_SZ,
            min_x: 9 * TILE_SZ,
            max_x: 15 * TILE_SZ,
            vx: -2,
        };

        self.spawn_item(0, CollectibleType::StarYabby, 12 * TILE_SZ, 11 * TILE_SZ, true);
        self.spawn_item(1, CollectibleType::YabbyRation, 12 * TILE_SZ, 5 * TILE_SZ, true);
    }
}
