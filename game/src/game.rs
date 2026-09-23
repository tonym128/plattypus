//! Main game loop and state machine for Plattypus MGS.
//! Integrates 3D GTE stealth gameplay, Soliton Radar, and CODEC radio communication.

use crate::audio::AudioManager;
use crate::codec::{
    CodecManager, ACT1_BOSS_DIALOGUE, ACT1_START_DIALOGUE, ACT2_START_DIALOGUE, ACT3_START_DIALOGUE,
    ACT4_START_DIALOGUE, INTRO_DIALOGUE, RADIO_TIPS_DIALOGUE,
};
use crate::dualshock::DualShockController;
use crate::entities::EntityManager;
use crate::level::{Act, CellType, Level, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};
use crate::renderer::Renderer;
use psx_gpu as gpu;
use psx_pad::{button, AnalogSticks, ButtonState, PadMode, PadState};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState {
    Title,
    IntroVideo,
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
    pub video: crate::video::VideoPlayer,
    pub prev_buttons: ButtonState,
    pub frame: u8,
    pub idle_timer: u16,
    pub memcard: crate::save::MemoryCardManager,
    pub save_data: crate::save::SaveData,
    pub dualshock: DualShockController,
    pub was_connected: bool,
}

impl Game {
    pub fn new() -> Self {
        let level = Level::new(Act::Act1Sanctuary);
        let platty = Platypus::new(level.player_start_x, level.player_start_z);
        let mut entities = EntityManager::new();
        entities.load_act(Act::Act1Sanctuary);
        let renderer = Renderer::new();
        // Upload title screen background texture to VRAM
        unsafe { crate::title_bg::upload_title_bg(); }
        let codec = CodecManager::new();
        let video = crate::video::VideoPlayer::new();
        let mut memcard = crate::save::MemoryCardManager::new();
        let save_data = memcard.load_from_slot1().unwrap_or_else(crate::save::SaveData::new);
        let mut dualshock = DualShockController::new();
        dualshock.init();

        Self {
            state: GameState::Title,
            level,
            platty,
            entities,
            renderer,
            codec,
            video,
            prev_buttons: ButtonState::NONE,
            frame: 0,
            idle_timer: 0,
            memcard,
            save_data,
            dualshock,
            was_connected: true,
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

        let (rumble_small, rumble_large) = self.platty.get_rumble_state();
        let pad = self.dualshock.poll(rumble_small, rumble_large);
        let buttons = pad.buttons;

        let is_connected = pad.is_connected();
        if is_connected && !self.was_connected {
            self.dualshock.init();
        }
        self.was_connected = is_connected;

        // Controller Disconnection Pause Screen during active gameplay
        let is_gameplay = matches!(self.state, GameState::Playing | GameState::InGameCodec);
        if !is_connected && is_gameplay {
            self.renderer.begin_frame();
            self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
            self.renderer.draw_hud(&self.platty, &self.entities, self.level.act);
            self.draw_controller_disconnected_overlay();
            self.prev_buttons = ButtonState::NONE;
            return;
        }

        let just_start = buttons.is_held(button::START) && !self.prev_buttons.is_held(button::START);
        let just_cross = buttons.is_held(button::CROSS) && !self.prev_buttons.is_held(button::CROSS);
        let just_select = buttons.is_held(button::SELECT) && !self.prev_buttons.is_held(button::SELECT);

        match self.state {
            GameState::Title => {
                if self.idle_timer == 0 {
                    AudioManager::play_cdda_title();
                    AudioManager::reset_cdda_tracking();
                }
                self.idle_timer += 1;

                // Grace period: wait ~1 second (60 frames) before accepting input
                // This avoids any controller initialization glitches on the first frames
                let any_button = self.idle_timer > 60 && is_connected && buttons.bits() != 0;
                if any_button {
                    AudioManager::stop_cdda();
                    self.video.start();
                    self.state = GameState::IntroVideo;
                } else if AudioManager::cdda_finished() {
                    // Enter Attract Demo Mode after CD track finishes
                    AudioManager::stop_cdda();
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::AttractDemo {
                        act: Act::Act1Sanctuary,
                        timer: 0,
                    };
                }

                self.renderer.begin_frame();
                self.renderer.draw_title_screen(self.frame);
            }
            GameState::IntroVideo => {
                let finished = self.video.update(&pad, &self.prev_buttons);
                if finished {
                    self.video.stop();
                    self.codec.start_conversation(INTRO_DIALOGUE);
                    self.state = GameState::IntroCodec;
                } else {
                    self.renderer.begin_frame();
                    self.video.draw(&self.renderer);
                }
            }
            GameState::AttractDemo { ref mut act, ref mut timer } => {
                // Any button press returns to title (only if controller is connected)
                let any_button = is_connected && buttons.bits() != 0;
                if any_button {
                    self.load_act(Act::Act1Sanctuary);
                    self.state = GameState::Title;
                    self.idle_timer = 0;
                } else {
                    *timer += 1;

                    // Simulated demo inputs for each act (gameplay only, no CODEC)
                    // UP = forward (+Z), DOWN = backward (-Z)
                    let sim_buttons = match *act {
                        Act::Act1Sanctuary | Act::Act1Boss => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::UP)
                            } else if *timer < 100 {
                                ButtonState::from_bits(button::RIGHT)
                            } else {
                                ButtonState::from_bits(button::CIRCLE | button::UP)
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
                    let is_sneaking = self.platty.state == PlayerState::Sneaking;
                    let is_submerged = self.platty.state == PlayerState::Submerged;

                    let demo_pad = PadState {
                        buttons: sim_buttons,
                        mode: PadMode::Digital,
                        sticks: AnalogSticks::CENTERED,
                        id_low: 0x41,
                    };

                    self.platty.update(&demo_pad, ButtonState::NONE, &self.level, &mut self.entities);
                    self.platty.health = self.platty.max_health;
                    self.entities.update(
                        self.level.act,
                        self.platty.x,
                        self.platty.y,
                        self.platty.z,
                        is_crawling,
                        is_sneaking,
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
                                *act = Act::Act1Boss;
                                self.load_act(Act::Act1Boss);
                            }
                            Act::Act1Boss => {
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
                                self.state = GameState::Title;
                                self.idle_timer = 0;
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
                    let is_sneaking = self.platty.state == PlayerState::Sneaking;
                    let is_submerged = self.platty.state == PlayerState::Submerged;

                    self.platty.update(&pad, self.prev_buttons, &self.level, &mut self.entities);
                    self.entities.update(
                        self.level.act,
                        self.platty.x,
                        self.platty.y,
                        self.platty.z,
                        is_crawling,
                        is_sneaking,
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
                    if self.level.act == Act::Act1Boss && self.entities.boss_mech.is_defeated() {
                        self.level.set_cell(11, 1, CellType::ExitBurrow);
                        self.level.set_cell(12, 1, CellType::ExitBurrow);
                        self.level.set_cell(13, 1, CellType::ExitBurrow);
                    }

                    let reached_exit = self.level.is_exit_at(self.platty.x, self.platty.z)
                        || (self.level.act == Act::Act1Boss && self.entities.boss_mech.is_defeated() && self.platty.z <= 2 * TILE_SZ)
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
                            Act::Act1Boss => ACT1_BOSS_DIALOGUE,
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
                    self.idle_timer = 0;
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
            Act::Act1Boss => crate::audio::BgmTrack::Boss,
            Act::Act2Bushland => crate::audio::BgmTrack::River,
            Act::Act3City => crate::audio::BgmTrack::City,
            Act::Act4Ocean => crate::audio::BgmTrack::Beach,
        };
        AudioManager::set_bgm(track);
    }

    fn draw_controller_disconnected_overlay(&self) {
        let box_x: i16 = 30;
        let box_y: i16 = 75;
        let box_w: u16 = 260;
        let box_h: u16 = 90;

        // Dark tactical overlay box
        gpu::draw_rect_flat(box_x, box_y, box_w, box_h, 12, 12, 20);
        // Red alert border
        gpu::draw_rect_flat(box_x, box_y, box_w, 2, 220, 40, 40);
        gpu::draw_rect_flat(box_x, box_y + box_h as i16 - 2, box_w, 2, 220, 40, 40);
        gpu::draw_rect_flat(box_x, box_y, 2, box_h, 220, 40, 40);
        gpu::draw_rect_flat(box_x + box_w as i16 - 2, box_y, 2, box_h, 220, 40, 40);

        // Header warning
        self.renderer.font.draw_text(box_x + 22, box_y + 14, "! CONTROLLER DISCONNECTED !", (255, 60, 60));
        self.renderer.font.draw_text(box_x + 18, box_y + 36, "PLEASE CONNECT A CONTROLLER", (220, 230, 240));
        self.renderer.font.draw_text(box_x + 52, box_y + 50, "TO CONTROLLER PORT 1", (220, 230, 240));
        self.renderer.font.draw_text(box_x + 28, box_y + 68, "[ DUALSHOCK / DIGITAL PAD ]", (120, 180, 220));
    }
}
