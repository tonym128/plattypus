//! Main game loop and state machine for Plattypus MGS.
//! Integrates 3D GTE stealth gameplay, Soliton Radar, and CODEC radio communication.

use crate::audio::{AudioManager, BgmTrack};
use crate::codec::{
    CodecManager, ACT1_START_DIALOGUE, ACT2_START_DIALOGUE, ACT3_START_DIALOGUE,
    ACT4_START_DIALOGUE, INTRO_DIALOGUE, RADIO_TIPS_DIALOGUE,
};
use crate::entities::EntityManager;
use crate::level::{Act, Level, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};
use crate::renderer::Renderer;
use psx_gpu as gpu;
use psx_pad::{button, poll_port1, ButtonState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState {
    Title,
    IntroCodec,
    StageIntroCodec,
    Playing,
    InGameCodec,
    StageClear,
    Ending,
    GameOver,
    AttractDemo { act: Act, timer: u16 },
}

pub struct Game {
    pub state: GameState,
    pub level: Level,
    pub platty: Platypus,
    pub entities: EntityManager,
    pub renderer: Renderer,
    pub codec: CodecManager,
    pub prev_buttons: ButtonState,
    pub frame: u8,
    pub idle_timer: u16,
    pub memcard: crate::save::MemoryCardManager,
    pub save_data: crate::save::SaveData,
}

impl Game {
    pub fn new() -> Self {
        let level = Level::new(Act::Act1Sanctuary);
        let platty = Platypus::new(level.player_start_x, level.player_start_z);
        let mut entities = EntityManager::new();
        entities.load_act(Act::Act1Sanctuary);
        let renderer = Renderer::new();
        let codec = CodecManager::new();
        let mut memcard = crate::save::MemoryCardManager::new();
        let save_data = memcard.load_from_slot1().unwrap_or_else(crate::save::SaveData::new);

        Self {
            state: GameState::Title,
            level,
            platty,
            entities,
            renderer,
            codec,
            prev_buttons: ButtonState::NONE,
            frame: 0,
            idle_timer: 0,
            memcard,
            save_data,
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.tick();
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        AudioManager::update();
        self.memcard.update();

        let pad = poll_port1();
        let buttons = pad.buttons;

        let just_start = buttons.is_held(button::START) && !self.prev_buttons.is_held(button::START);
        let just_cross = buttons.is_held(button::CROSS) && !self.prev_buttons.is_held(button::CROSS);
        let just_select = buttons.is_held(button::SELECT) && !self.prev_buttons.is_held(button::SELECT);

        match self.state {
            GameState::Title => {
                AudioManager::set_bgm(BgmTrack::Title);
                self.idle_timer += 1;
                if just_start || just_cross {
                    // Launch Intro CODEC transmission from Burrow Command!
                    self.codec.start_conversation(INTRO_DIALOGUE);
                    self.state = GameState::IntroCodec;
                } else if self.idle_timer > 90 {
                    // Enter Attract Demo Mode to showcase stages
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::AttractDemo {
                        act: Act::Act1Sanctuary,
                        timer: 0,
                    };
                }

                self.renderer.begin_frame();
                self.renderer.draw_title_screen(self.frame);
            }
            GameState::AttractDemo { ref mut act, ref mut timer } => {
                if just_start {
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::Title;
                    self.idle_timer = 0;
                } else {
                    *timer += 1;

                    // Simulated demo inputs for each act
                    let sim_buttons = match *act {
                        Act::Act1Sanctuary => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::DOWN)
                            } else if *timer < 100 {
                                ButtonState::from_bits(button::RIGHT)
                            } else {
                                ButtonState::from_bits(button::CIRCLE | button::DOWN)
                            }
                        }
                        Act::Act2Bushland => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::LEFT)
                            } else if *timer < 90 {
                                ButtonState::from_bits(button::CROSS)
                            } else {
                                ButtonState::from_bits(button::RIGHT)
                            }
                        }
                        Act::Act3City => {
                            if (*timer / 25) % 2 == 0 {
                                ButtonState::from_bits(button::UP)
                            } else {
                                ButtonState::NONE
                            }
                        }
                        Act::Act4Ocean => {
                            if (*timer / 30) % 2 == 0 {
                                ButtonState::from_bits(button::CROSS | button::RIGHT | button::UP)
                            } else {
                                ButtonState::from_bits(button::RIGHT | button::UP)
                            }
                        }
                    };

                    let is_crawling = self.platty.state == PlayerState::BellyCrawl;
                    let is_submerged = self.platty.state == PlayerState::Submerged;

                    self.platty.update(sim_buttons, ButtonState::NONE, &self.level, &mut self.entities);
                    self.platty.health = self.platty.max_health;
                    self.entities.update(
                        self.level.act,
                        self.platty.x,
                        self.platty.y,
                        self.platty.z,
                        is_crawling,
                        is_submerged,
                        &self.level,
                    );
                    self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

                    // Render 3D Scene & HUD
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_hud(&self.platty, &self.entities, self.level.act);

                    // Cycle to next demo act every 180 frames (3 seconds)
                    if *timer >= 180 {
                        *timer = 0;
                        match *act {
                            Act::Act1Sanctuary => {
                                *act = Act::Act2Bushland;
                                self.load_act(Act::Act2Bushland);
                            }
                            Act::Act2Bushland => {
                                *act = Act::Act3City;
                                self.load_act(Act::Act3City);
                            }
                            Act::Act3City => {
                                *act = Act::Act4Ocean;
                                self.load_act(Act::Act4Ocean);
                            }
                            Act::Act4Ocean => {
                                self.load_act(Act::Act1Sanctuary);
                                self.codec.start_conversation(INTRO_DIALOGUE);
                                self.state = GameState::IntroCodec;
                            }
                        }
                    }
                }
            }
            GameState::IntroCodec => {
                self.codec.update();
                if just_cross || just_start {
                    if self.codec.on_action_button() {
                        // Intro finished, start Act 1!
                        self.load_act(Act::Act1Sanctuary);
                        self.codec.start_conversation(ACT1_START_DIALOGUE);
                        self.state = GameState::StageIntroCodec;
                    }
                }

                self.renderer.begin_frame();
                self.codec.draw(&self.renderer.font);
            }
            GameState::StageIntroCodec => {
                self.codec.update();
                if just_cross || just_start {
                    if self.codec.on_action_button() {
                        self.state = GameState::Playing;
                        AudioManager::play_jump();
                    }
                }

                self.renderer.begin_frame();
                self.codec.draw(&self.renderer.font);
            }
            GameState::Playing => {
                // Check if player presses SELECT to open in-game CODEC radio!
                if just_select {
                    self.codec.start_conversation(RADIO_TIPS_DIALOGUE);
                    self.state = GameState::InGameCodec;
                } else {
                    // Update player and 3D entities
                    let is_crawling = self.platty.state == PlayerState::BellyCrawl;
                    let is_submerged = self.platty.state == PlayerState::Submerged;

                    self.platty.update(buttons, self.prev_buttons, &self.level, &mut self.entities);
                    self.entities.update(
                        self.level.act,
                        self.platty.x,
                        self.platty.y,
                        self.platty.z,
                        is_crawling,
                        is_submerged,
                        &self.level,
                    );

                    if self.platty.screen_shake > 0 {
                        self.renderer.screen_shake = self.platty.screen_shake as i16;
                        self.platty.screen_shake = 0;
                    }
                    self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

                    // Check Game Over (health 0)
                    if self.platty.health == 0 {
                        self.state = GameState::GameOver;
                        AudioManager::play_hit();
                    }

                    // Check exit or goal reached
                    let reached_exit = self.level.is_exit_at(self.platty.x, self.platty.z)
                        || (self.level.act == Act::Act2Bushland && self.platty.z >= 21 * TILE_SZ)
                        || (self.level.act == Act::Act3City && self.platty.z <= 3 * TILE_SZ);

                    if reached_exit {
                        // Automatically save progress & high score to Memory Card
                        self.save_data.unlocked_act = (self.level.act as u8 + 1).max(self.save_data.unlocked_act);
                        self.save_data.highest_score = self.save_data.highest_score.max(self.platty.score);
                        self.save_data.total_yabbies = self.save_data.total_yabbies.saturating_add(self.platty.yabbies_collected as u16);
                        self.memcard.save_to_slot1(&self.save_data);

                        if self.level.act == Act::Act4Ocean {
                            self.state = GameState::Ending;
                            AudioManager::play_fanfare();
                        } else {
                            self.state = GameState::StageClear;
                            AudioManager::play_fanfare();
                        }
                    }

                    // Render 3D World & HUD
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_hud(&self.platty, &self.entities, self.level.act);

                    // Memory Card tactical OSD message
                    match self.memcard.status {
                        crate::save::SaveStatus::Saving => {
                            gpu::draw_rect_flat(100, 214, 120, 18, 10, 25, 40);
                            self.renderer.font.draw_text(106, 218, "SAVING TO MEM CARD...", (100, 220, 255));
                        }
                        crate::save::SaveStatus::SaveSuccess => {
                            gpu::draw_rect_flat(96, 214, 128, 18, 10, 35, 20);
                            self.renderer.font.draw_text(102, 218, "MISSION PROGRESS SAVED", (120, 255, 140));
                        }
                        crate::save::SaveStatus::SaveErrorNoCard => {
                            gpu::draw_rect_flat(88, 214, 144, 18, 35, 10, 10);
                            self.renderer.font.draw_text(94, 218, "NO MEMORY CARD IN SLOT 1", (255, 160, 160));
                        }
                        _ => {}
                    }
                }
            }
            GameState::InGameCodec => {
                self.codec.update();
                if just_cross || just_select || just_start {
                    if self.codec.on_action_button() {
                        self.state = GameState::Playing;
                    }
                }

                self.renderer.begin_frame();
                self.codec.draw(&self.renderer.font);
            }
            GameState::StageClear => {
                if just_cross || just_start {
                    if let Some(next_act) = self.level.act.next() {
                        self.load_act(next_act);
                        let briefing = match next_act {
                            Act::Act2Bushland => ACT2_START_DIALOGUE,
                            Act::Act3City => ACT3_START_DIALOGUE,
                            Act::Act4Ocean => ACT4_START_DIALOGUE,
                            _ => ACT1_START_DIALOGUE,
                        };
                        self.codec.start_conversation(briefing);
                        self.state = GameState::StageIntroCodec;
                    } else {
                        self.state = GameState::Ending;
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_stage_clear(self.level.act, self.platty.score, self.platty.yabbies_collected);
            }
            GameState::Ending => {
                if just_start || just_cross {
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::Title;
                }

                self.renderer.begin_frame();
                self.renderer.draw_ending(self.frame);
            }
            GameState::GameOver => {
                // MGS Classic Game Over: "PLATTY? PLATTY? PLATTYYYYY!"
                if just_cross || just_start {
                    // Retry current stage
                    let act = self.level.act;
                    self.load_act(act);
                    self.state = GameState::Playing;
                }

                self.renderer.begin_frame();
                gpu::draw_rect_flat(0, 0, 320, 240, 16, 4, 4);
                self.renderer.font.draw_text(115, 80, "GAME OVER", (255, 40, 40));
                self.renderer.font.draw_text(60, 110, "BURROW HQ: PLATTY? PLATTYYYY!", (255, 220, 220));
                self.renderer.font.draw_text(75, 150, "PRESS CROSS TO RETRY MISSION", (255, 255, 255));
            }
        }

        self.prev_buttons = buttons;
    }

    fn load_act(&mut self, act: Act) {
        self.level = Level::new(act);
        self.platty.reset_position(self.level.player_start_x, self.level.player_start_z);
        self.entities.load_act(act);
        self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);
        let track = match act {
            Act::Act1Sanctuary => crate::audio::BgmTrack::Stealth,
            Act::Act2Bushland => crate::audio::BgmTrack::River,
            Act::Act3City => crate::audio::BgmTrack::City,
            Act::Act4Ocean => crate::audio::BgmTrack::Beach,
        };
        AudioManager::set_bgm(track);
    }
}
