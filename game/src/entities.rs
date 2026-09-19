//! Entities, items, NPCs, enemies, and particle systems for Plattypus.

use crate::fixed::Fixed;

pub const MAX_COLLECTIBLES: usize = 24;
pub const MAX_ENEMIES: usize = 12;
pub const MAX_PARTICLES: usize = 32;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CollectibleType {
    Yabby,       // Freshwater crayfish (heals 1 HP, 100 pts)
    LetterPage,  // Lost letter fragments (250 pts)
    BuriedYabby, // Secret buried yabby (revealed via electro-sense, heals 2 HP, 300 pts)
}

#[derive(Copy, Clone, Debug)]
pub struct Collectible {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub kind: CollectibleType,
    pub bob_timer: u8,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EnemyType {
    Zookeeper, // Has a flashlight beam in Act 1
    Wombat,    // Charges along the ground in Act 2
    Pigeon,    // Flutters horizontally in Act 3
    Crab,      // Scuttles along the sand in Act 4
}

#[derive(Copy, Clone, Debug)]
pub struct Enemy {
    pub active: bool,
    pub x: Fixed,
    pub y: Fixed,
    pub vx: Fixed,
    pub kind: EnemyType,
    pub min_x: i32,
    pub max_x: i32,
    pub facing_right: bool,
    pub stun_timer: u8,
}

#[derive(Copy, Clone, Debug)]
pub struct Particle {
    pub active: bool,
    pub x: i32,
    pub y: i32,
    pub vx: i16,
    pub vy: i16,
    pub life: u8,
    pub color: (u8, u8, u8),
    pub size: u8,
}

pub struct EntityManager {
    pub collectibles: [Collectible; MAX_COLLECTIBLES],
    pub enemies: [Enemy; MAX_ENEMIES],
    pub particles: [Particle; MAX_PARTICLES],
    pub next_particle: usize,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            collectibles: [Collectible {
                active: false,
                x: 0,
                y: 0,
                kind: CollectibleType::Yabby,
                bob_timer: 0,
            }; MAX_COLLECTIBLES],
            enemies: [Enemy {
                active: false,
                x: Fixed::ZERO,
                y: Fixed::ZERO,
                vx: Fixed::ZERO,
                kind: EnemyType::Zookeeper,
                min_x: 0,
                max_x: 0,
                facing_right: true,
                stun_timer: 0,
            }; MAX_ENEMIES],
            particles: [Particle {
                active: false,
                x: 0,
                y: 0,
                vx: 0,
                vy: 0,
                life: 0,
                color: (255, 255, 255),
                size: 2,
            }; MAX_PARTICLES],
            next_particle: 0,
        }
    }

    pub fn spawn_for_act(&mut self, act: crate::level::Act) {
        // Clear all
        for c in self.collectibles.iter_mut() {
            c.active = false;
        }
        for e in self.enemies.iter_mut() {
            e.active = false;
        }
        for p in self.particles.iter_mut() {
            p.active = false;
        }

        match act {
            crate::level::Act::Act1Sanctuary => {
                // Yabbies in ponds
                self.add_collectible(8 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(12 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(22 * 16, 9 * 16, CollectibleType::LetterPage);
                self.add_collectible(42 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(48 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(62 * 16, 8 * 16, CollectibleType::LetterPage);
                self.add_collectible(69 * 16, 12 * 16, CollectibleType::BuriedYabby); // Hidden behind mud wall!
                self.add_collectible(84 * 16, 11 * 16, CollectibleType::Yabby);

                // Zookeepers patrolling
                self.add_enemy(20 * 16, 12 * 16, 18 * 16, 28 * 16, EnemyType::Zookeeper);
                self.add_enemy(58 * 16, 12 * 16, 54 * 16, 68 * 16, EnemyType::Zookeeper);
            }
            crate::level::Act::Act2Bushland => {
                self.add_collectible(15 * 16, 9 * 16, CollectibleType::Yabby);
                self.add_collectible(30 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(38 * 16, 11 * 16, CollectibleType::Yabby);
                self.add_collectible(47 * 16, 13 * 16, CollectibleType::BuriedYabby); // Hidden in burrow!
                self.add_collectible(50 * 16, 10 * 16, CollectibleType::LetterPage);
                self.add_collectible(70 * 16, 10 * 16, CollectibleType::LetterPage);
                self.add_collectible(88 * 16, 12 * 16, CollectibleType::Yabby);

                // Wombats
                self.add_enemy(32 * 16, 13 * 16, 28 * 16, 42 * 16, EnemyType::Wombat);
                self.add_enemy(68 * 16, 10 * 16, 66 * 16, 76 * 16, EnemyType::Wombat);
            }
            crate::level::Act::Act3City => {
                self.add_collectible(14 * 16, 8 * 16, CollectibleType::LetterPage);
                self.add_collectible(25 * 16, 8 * 16, CollectibleType::Yabby);
                self.add_collectible(38 * 16, 5 * 16, CollectibleType::LetterPage);
                self.add_collectible(54 * 16, 11 * 16, CollectibleType::Yabby);
                self.add_collectible(67 * 16, 12 * 16, CollectibleType::BuriedYabby); // Hidden in drain silt!
                self.add_collectible(76 * 16, 11 * 16, CollectibleType::Yabby);
                self.add_collectible(82 * 16, 12 * 16, CollectibleType::Yabby);

                // Pigeons
                self.add_enemy(12 * 16, 7 * 16, 10 * 16, 20 * 16, EnemyType::Pigeon);
                self.add_enemy(34 * 16, 5 * 16, 32 * 16, 42 * 16, EnemyType::Pigeon);
            }
            crate::level::Act::Act4Ocean => {
                self.add_collectible(12 * 16, 11 * 16, CollectibleType::Yabby);
                self.add_collectible(32 * 16, 12 * 16, CollectibleType::Yabby);
                self.add_collectible(40 * 16, 11 * 16, CollectibleType::Yabby);
                self.add_collectible(55 * 16, 10 * 16, CollectibleType::LetterPage);
                self.add_collectible(74 * 16, 7 * 16, CollectibleType::LetterPage);
                self.add_collectible(96 * 16, 13 * 16, CollectibleType::BuriedYabby); // Hidden family nest treasure!
                self.add_collectible(102 * 16, 12 * 16, CollectibleType::Yabby);

                // Crabs
                self.add_enemy(10 * 16, 11 * 16, 8 * 16, 16 * 16, EnemyType::Crab);
                self.add_enemy(52 * 16, 10 * 16, 50 * 16, 62 * 16, EnemyType::Crab);
            }
        }
    }

    pub fn add_collectible(&mut self, x: i32, y: i32, kind: CollectibleType) {
        for c in self.collectibles.iter_mut() {
            if !c.active {
                c.active = true;
                c.x = x;
                c.y = y;
                c.kind = kind;
                c.bob_timer = 0;
                break;
            }
        }
    }

    pub fn add_enemy(&mut self, x: i32, y: i32, min_x: i32, max_x: i32, kind: EnemyType) {
        for e in self.enemies.iter_mut() {
            if !e.active {
                e.active = true;
                e.x = Fixed::from_int(x);
                e.y = Fixed::from_int(y);
                e.vx = Fixed::from_fraction(1, 2);
                e.kind = kind;
                e.min_x = min_x;
                e.max_x = max_x;
                e.facing_right = true;
                e.stun_timer = 0;
                break;
            }
        }
    }

    pub fn spawn_particle(&mut self, x: i32, y: i32, vx: i16, vy: i16, life: u8, color: (u8, u8, u8), size: u8) {
        let p = &mut self.particles[self.next_particle];
        p.active = true;
        p.x = x;
        p.y = y;
        p.vx = vx;
        p.vy = vy;
        p.life = life;
        p.color = color;
        p.size = size;
        self.next_particle = (self.next_particle + 1) % MAX_PARTICLES;
    }

    pub fn update(&mut self) {
        // Collectibles bobbing
        for c in self.collectibles.iter_mut() {
            if c.active {
                c.bob_timer = c.bob_timer.wrapping_add(1);
            }
        }

        // Enemies patrol
        for e in self.enemies.iter_mut() {
            if !e.active {
                continue;
            }
            if e.stun_timer > 0 {
                e.stun_timer -= 1;
                continue;
            }

            e.x += e.vx;
            let current_x = e.x.to_int();
            if e.facing_right && current_x >= e.max_x {
                e.facing_right = false;
                e.vx = -e.vx.abs();
            } else if !e.facing_right && current_x <= e.min_x {
                e.facing_right = true;
                e.vx = e.vx.abs();
            }
        }

        // Particles physics
        for p in self.particles.iter_mut() {
            if !p.active {
                continue;
            }
            p.x += p.vx as i32;
            p.y += p.vy as i32;
            if p.life > 0 {
                p.life -= 1;
            } else {
                p.active = false;
            }
        }
    }
}
