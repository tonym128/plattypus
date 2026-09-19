//! Main game coordinator and state machine for Plattypus.

use crate::audio::AudioManager;
use crate::entities::EntityManager;
use crate::level::{Act, Level};
use crate::platypus::Platypus;
use crate::renderer::Renderer;
use psx_pad::{button, poll_port1, ButtonState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState {
    Title,
    Letter,
    StageIntro(u8),
    Playing,
    StageClear,
    Ending,
}

pub struct Game {
    pub state: GameState,
    pub level: Level,
    pub platty: Platypus,
    pub entities: EntityManager,
    pub renderer: Renderer,
    pub prev_buttons: ButtonState,
    pub frame: u8,
}

impl Game {
    pub fn new() -> Self {
        let level = Level::new(Act::Act1Sanctuary);
        let platty = Platypus::new(level.player_start_x, level.player_start_y);
        let mut entities = EntityManager::new();
        entities.spawn_for_act(Act::Act1Sanctuary);
        let renderer = Renderer::new();

        Self {
            state: GameState::Title,
            level,
            platty,
            entities,
            renderer,
            prev_buttons: ButtonState::NONE,
            frame: 0,
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.tick();
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        let pad = poll_port1();
        let buttons = pad.buttons;

        let just_start = buttons.is_held(button::START) && !self.prev_buttons.is_held(button::START);
        let just_cross = buttons.is_held(button::CROSS) && !self.prev_buttons.is_held(button::CROSS);

        match self.state {
            GameState::Title => {
                if just_start || just_cross {
                    self.state = GameState::Letter;
                    AudioManager::play_fanfare();
                }

                self.renderer.begin_frame();
                self.renderer.draw_title_screen(self.frame);
            }
            GameState::Letter => {
                if just_cross || just_start {
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::StageIntro(90);
                    AudioManager::play_jump();
                }

                self.renderer.begin_frame();
                self.renderer.draw_letter_intro(self.frame);
            }
            GameState::StageIntro(timer) => {
                if timer > 0 {
                    self.state = GameState::StageIntro(timer - 1);
                } else {
                    self.state = GameState::Playing;
                }

                self.renderer.begin_frame();
                self.renderer.draw_background(self.level.act, self.frame);
                self.renderer.draw_level(&self.level, self.frame);
                self.renderer.draw_platypus(&self.platty);
                self.renderer.draw_hud(&self.platty, self.level.act);

                // Stage intro banner
                self.renderer.font.draw_text(60, 100, self.level.act.title(), (255, 230, 80));
                self.renderer.font.draw_text(45, 120, self.level.act.subtitle(), (200, 240, 255));
            }
            GameState::Playing => {
                // Update player and entities
                self.platty.update(buttons, self.prev_buttons, &mut self.level, &mut self.entities);
                self.entities.update();
                self.renderer.update_camera(self.platty.x);

                // Check death (health 0 or fell off screen)
                if self.platty.health == 0 || self.platty.y.to_int() > 240 {
                    // Respawn at stage start
                    self.platty.health = 3;
                    self.platty.reset_position(self.level.player_start_x, self.level.player_start_y);
                    AudioManager::play_hit();
                }

                // Check exit reached
                let px = self.platty.x.to_int();
                let py = self.platty.y.to_int();
                if self.level.is_exit_at(px + 8, py + 8) {
                    if self.level.act == Act::Act4Ocean {
                        self.state = GameState::Ending;
                        AudioManager::play_fanfare();
                    } else {
                        self.state = GameState::StageClear;
                        AudioManager::play_fanfare();
                    }
                }

                // Render frame
                self.renderer.begin_frame();
                self.renderer.draw_background(self.level.act, self.frame);
                self.renderer.draw_level(&self.level, self.frame);
                self.renderer.draw_entities(&self.entities, &self.platty);
                self.renderer.draw_platypus(&self.platty);
                self.renderer.draw_hud(&self.platty, self.level.act);
            }
            GameState::StageClear => {
                if just_cross || just_start {
                    if let Some(next_act) = self.level.act.next() {
                        self.load_act(next_act);
                        self.state = GameState::StageIntro(90);
                    } else {
                        self.state = GameState::Ending;
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_background(self.level.act, self.frame);
                self.renderer.draw_level(&self.level, self.frame);
                self.renderer.draw_stage_clear(self.level.act, self.platty.score, self.platty.yabbies_collected);
            }
            GameState::Ending => {
                if just_start || just_cross {
                    // Return to title after beating game
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::Title;
                }

                self.renderer.begin_frame();
                self.renderer.draw_ending(self.frame);
            }
        }

        self.prev_buttons = buttons;
    }

    fn load_act(&mut self, act: Act) {
        self.level = Level::new(act);
        self.platty.reset_position(self.level.player_start_x, self.level.player_start_y);
        self.entities.spawn_for_act(act);
        self.renderer.camera_x = 0;
    }
}
