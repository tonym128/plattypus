//! Main game loop and state machine for Plattypus MGS.
//! Integrates 3D GTE stealth gameplay, Soliton Radar, and CODEC radio communication.

use crate::audio::AudioManager;
use crate::codec::{CodecManager, INTRO_DIALOGUE, ACT1_1_DIALOGUE, get_act_dialogue};
use crate::dualshock::DualShockController;
use crate::entities::EntityManager;
use crate::level::{Act, Level, TILE_SZ};
use crate::platypus::{PlayerState, Platypus};
use crate::renderer::Renderer;
use psx_gpu as gpu;
use psx_pad::{button, AnalogSticks, ButtonState, PadMode, PadState};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionStats {
    pub play_time_frames: u32,
    pub alerts_count: u16,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState {
    Title,
    VrMenu,
    OptionsMenu,
    MissionDebriefing { timer: u16, codename: crate::save::Codename },
    IntroVideo,
    OutroVideo { codename: crate::save::Codename },
    IntroCodec,
    StageIntroCodec,
    Playing,
    InGameCodec,
    StageClear,
    Ending { codename: Option<crate::save::Codename> },
    GameOver,
    Paused,
    AttractDemo { act: Act, timer: u16 },
    BossIntroCutscene { act: Act, timer: u16 },
    ChapterTitleCard { act: Act, timer: u16 },
    StageSelect,
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
    pub mission_stats: MissionStats,
    pub title_selection: usize,
    pub vr_selection: usize,
    pub stage_selection: usize,
    pub options_selection: usize,
    pub pause_selection: usize,
    pub stage_time_frames: u32,
    pub stage_alerts: u16,
    pub stage_start_takedowns: u16,
}

impl Game {
    pub fn new() -> Self {
        let level = Level::new(Act::Act1_1Drainage);
        let platty = Platypus::new(level.player_start_x, level.player_start_z);
        let mut entities = EntityManager::new();
        entities.load_act(Act::Act1_1Drainage);
        let mut renderer = Renderer::new();
        // Upload title screen background texture to VRAM
        unsafe { crate::title_bg::upload_title_bg(); }
        let codec = CodecManager::new();
        let video = crate::video::VideoPlayer::new();
        let mut memcard = crate::save::MemoryCardManager::new();
        let save_data = memcard.load_from_slot1().unwrap_or_else(crate::save::SaveData::new);
        renderer.costume = save_data.selected_costume;
        renderer.wireframe = save_data.wireframe_enabled != 0;
        renderer.language = save_data.language;
        renderer.screen_offset_x = save_data.screen_offset_x;
        renderer.screen_offset_y = save_data.screen_offset_y;
        if save_data.pal_mode == 0 {
            renderer.video_mode = psx_gpu::VideoMode::Ntsc;
        } else if save_data.pal_mode == 1 {
            renderer.video_mode = psx_gpu::VideoMode::Pal;
        }
        renderer.apply_display_offset();

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
            mission_stats: MissionStats::default(),
            title_selection: 0,
            vr_selection: 0,
            stage_selection: 0,
            options_selection: 0,
            pause_selection: 0,
            stage_time_frames: 0,
            stage_alerts: 0,
            stage_start_takedowns: 0,
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
        let is_gameplay = matches!(self.state, GameState::Playing | GameState::InGameCodec | GameState::Paused);
        if !is_connected && is_gameplay {
            self.renderer.begin_frame();
            self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
            self.renderer.draw_hud(&self.platty, &self.entities, &self.level, self.frame);
            self.draw_controller_disconnected_overlay();
            self.prev_buttons = ButtonState::NONE;
            return;
        }

        let just_start = buttons.is_held(button::START) && !self.prev_buttons.is_held(button::START);
        let just_cross = buttons.is_held(button::CROSS) && !self.prev_buttons.is_held(button::CROSS);
        let just_circle = buttons.is_held(button::CIRCLE) && !self.prev_buttons.is_held(button::CIRCLE);
        let just_select = buttons.is_held(button::SELECT) && !self.prev_buttons.is_held(button::SELECT);
        let just_up = buttons.is_held(button::UP) && !self.prev_buttons.is_held(button::UP);
        let just_down = buttons.is_held(button::DOWN) && !self.prev_buttons.is_held(button::DOWN);
        let just_left = buttons.is_held(button::LEFT) && !self.prev_buttons.is_held(button::LEFT);
        let just_right = buttons.is_held(button::RIGHT) && !self.prev_buttons.is_held(button::RIGHT);

        match self.state {
            GameState::Title => {
                if self.idle_timer == 0 {
                    AudioManager::play_cdda_title();
                    AudioManager::reset_cdda_tracking();
                }
                self.idle_timer += 1;

                let has_save = self.save_data.unlocked_act > 0;
                let max_selection = if has_save { 3 } else { 2 };

                if self.idle_timer > 30 && is_connected {
                    if just_up {
                        self.title_selection = if self.title_selection == 0 { max_selection } else { self.title_selection - 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_down {
                        self.title_selection = if self.title_selection >= max_selection { 0 } else { self.title_selection + 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_cross || just_start {
                        AudioManager::play_jump();
                        if has_save {
                            match self.title_selection {
                                0 => {
                                    if self.save_data.unlocked_act >= 11 {
                                        // Unlocked Post-Campaign Stage Select
                                        AudioManager::stop_cdda();
                                        self.state = GameState::StageSelect;
                                        self.stage_selection = 0;
                                    } else {
                                        // Continue Campaign from unlocked act (clamped to campaign acts 0..=11)
                                        AudioManager::stop_cdda();
                                        let act_idx = self.save_data.unlocked_act.min(11);
                                        let act = Act::from_u8(act_idx);
                                        self.load_act(act);
                                        let briefing = get_act_dialogue(act);
                                        self.codec.start_conversation(briefing);
                                        self.state = GameState::StageIntroCodec;
                                    }
                                }
                                1 => {
                                    // New Campaign Infiltration
                                    AudioManager::stop_cdda();
                                    self.mission_stats = MissionStats::default();
                                    self.platty.reset_for_new_game();
                                    self.video.start();
                                    self.state = GameState::IntroVideo;
                                }
                                2 => {
                                    // VR Training Simulator
                                    AudioManager::stop_cdda();
                                    self.state = GameState::VrMenu;
                                    self.vr_selection = 0;
                                }
                                _ => {
                                    // Special Options & Gear
                                    self.state = GameState::OptionsMenu;
                                    self.options_selection = 0;
                                }
                            }
                        } else {
                            match self.title_selection {
                                0 => {
                                    // Campaign Infiltration
                                    AudioManager::stop_cdda();
                                    self.mission_stats = MissionStats::default();
                                    self.platty.reset_for_new_game();
                                    self.video.start();
                                    self.state = GameState::IntroVideo;
                                }
                                1 => {
                                    // VR Training Simulator
                                    AudioManager::stop_cdda();
                                    self.state = GameState::VrMenu;
                                    self.vr_selection = 0;
                                }
                                _ => {
                                    // Special Options & Gear
                                    self.state = GameState::OptionsMenu;
                                    self.options_selection = 0;
                                }
                            }
                        }
                    } else if AudioManager::cdda_finished() {
                        AudioManager::stop_cdda();
                        self.load_act(Act::Act1_1Drainage);
                        self.state = GameState::AttractDemo {
                            act: Act::Act1_1Drainage,
                            timer: 0,
                        };
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_title_screen(self.frame, self.title_selection, &self.save_data);
            }
            GameState::StageSelect => {
                if is_connected {
                    if just_up {
                        if self.stage_selection % 6 == 0 {
                            self.stage_selection += 5;
                        } else {
                            self.stage_selection -= 1;
                        }
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_down {
                        if self.stage_selection % 6 == 5 {
                            self.stage_selection -= 5;
                        } else {
                            self.stage_selection += 1;
                        }
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_left || just_right {
                        if self.stage_selection < 6 {
                            self.stage_selection += 6;
                        } else {
                            self.stage_selection -= 6;
                        }
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_circle {
                        self.state = GameState::Title;
                        self.idle_timer = 0;
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_cross || just_start {
                        AudioManager::stop_cdda();
                        let target_act = Act::from_u8(self.stage_selection as u8);
                        self.load_act(target_act);
                        if target_act.is_boss() {
                            self.state = GameState::BossIntroCutscene { act: target_act, timer: 0 };
                        } else {
                            let briefing = get_act_dialogue(target_act);
                            self.codec.start_conversation(briefing);
                            self.state = GameState::StageIntroCodec;
                        }
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_stage_select_menu(self.stage_selection, self.save_data.unlocked_act);
            }
            GameState::VrMenu => {
                if is_connected {
                    if just_up {
                        self.vr_selection = if self.vr_selection == 0 { 3 } else { self.vr_selection - 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_down {
                        self.vr_selection = if self.vr_selection == 3 { 0 } else { self.vr_selection + 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_circle {
                        self.state = GameState::Title;
                        self.idle_timer = 0;
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_cross || just_start {
                        let vr_act = match self.vr_selection {
                            0 => Act::VrSneaking,
                            1 => Act::VrCqc,
                            2 => Act::VrSonar,
                            _ => Act::VrSpeed,
                        };
                        self.load_act(vr_act);
                        self.state = GameState::Playing;
                        AudioManager::play_jump();
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_vr_menu(self.vr_selection, self.save_data.vr_cleared);
            }
            GameState::OptionsMenu => {
                if is_connected {
                    if just_up {
                        self.options_selection = if self.options_selection == 0 { 4 } else { self.options_selection - 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_down {
                        self.options_selection = if self.options_selection == 4 { 0 } else { self.options_selection + 1 };
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_circle {
                        self.state = GameState::Title;
                        self.idle_timer = 0;
                        AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                    } else if just_left || just_right {
                        match self.options_selection {
                            0 => {
                                // Cycle unlocked costumes: 0 (Default), 1 (Tuxedo), 2 (Stealth Camo)
                                let mut next_costume = self.save_data.selected_costume;
                                for _ in 0..3 {
                                    next_costume = (next_costume + 1) % 3;
                                    if next_costume == 0
                                        || (next_costume == 1 && self.save_data.tuxedo_unlocked != 0)
                                        || (next_costume == 2 && self.save_data.camo_unlocked != 0)
                                    {
                                        break;
                                    }
                                }
                                self.save_data.selected_costume = next_costume;
                                self.renderer.costume = next_costume;
                                self.memcard.save_to_slot1(&self.save_data);
                                AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                            }
                            1 => {
                                // Toggle Wireframe mode if unlocked
                                if self.save_data.wireframe_unlocked != 0 {
                                    self.save_data.wireframe_enabled = if self.save_data.wireframe_enabled == 0 { 1 } else { 0 };
                                    self.renderer.wireframe = self.save_data.wireframe_enabled != 0;
                                    self.memcard.save_to_slot1(&self.save_data);
                                    AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                                }
                            }
                            2 => {
                                // Multi-language selector: 0 (EN), 1 (FR), 2 (DE), 3 (ES), 4 (JP)
                                if just_right {
                                    self.save_data.language = (self.save_data.language + 1) % 5;
                                } else {
                                    self.save_data.language = if self.save_data.language == 0 { 4 } else { self.save_data.language - 1 };
                                }
                                self.renderer.language = self.save_data.language;
                                self.memcard.save_to_slot1(&self.save_data);
                                AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                            }
                            3 => {
                                // Video Standard: 0 (NTSC 60Hz), 1 (PAL 50Hz), 2 (Auto Detect)
                                self.save_data.pal_mode = (self.save_data.pal_mode + 1) % 3;
                                let mode = match self.save_data.pal_mode {
                                    0 => psx_gpu::VideoMode::Ntsc,
                                    1 => psx_gpu::VideoMode::Pal,
                                    _ => crate::save::detect_console_region().0,
                                };
                                self.renderer.video_mode = mode;
                                self.renderer.apply_display_offset();
                                self.memcard.save_to_slot1(&self.save_data);
                                AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                            }
                            4 => {
                                // Screen V-Center Offset (-16..16 scanlines)
                                if just_left {
                                    self.save_data.screen_offset_y = (self.save_data.screen_offset_y - 1).max(-16);
                                } else {
                                    self.save_data.screen_offset_y = (self.save_data.screen_offset_y + 1).min(16);
                                }
                                self.renderer.screen_offset_y = self.save_data.screen_offset_y;
                                self.renderer.apply_display_offset();
                                self.memcard.save_to_slot1(&self.save_data);
                                AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                            }
                            _ => {}
                        }
                    }
                }

                self.renderer.begin_frame();
                self.renderer.draw_options_menu(&self.save_data, self.options_selection);
            }
            GameState::MissionDebriefing { ref mut timer, codename } => {
                *timer = timer.saturating_add(1);

                if *timer > 45 && (just_cross || just_start) {
                    self.state = GameState::Ending { codename: Some(codename) };
                }

                self.renderer.begin_frame();
                self.renderer.draw_debriefing_screen(
                    self.mission_stats.play_time_frames / 60,
                    self.mission_stats.alerts_count,
                    self.platty.takedowns,
                    self.platty.total_damage,
                    self.platty.yabbies_collected,
                    codename,
                );
            }
            GameState::IntroVideo => {
                let finished = self.video.update(&pad, &self.prev_buttons);
                if finished {
                    self.video.stop();
                    self.codec.start_conversation(INTRO_DIALOGUE);
                    self.state = GameState::IntroCodec;
                } else if self.video.needs_redraw() {
                    self.renderer.begin_frame();
                    self.video.draw(&self.renderer);
                } else {
                    // Hold the displayed buffer until the next 15 fps video
                    // frame is ready; swapping every VBlank flashes the stale
                    // alternate buffer between uploads.
                    psx_rt::interrupts::wait_vblank();
                }
            }
            GameState::OutroVideo { codename } => {
                let finished = self.video.update(&pad, &self.prev_buttons);
                if finished {
                    self.video.stop();
                    AudioManager::play_fanfare();
                    self.state = GameState::MissionDebriefing { timer: 0, codename };
                } else if self.video.needs_redraw() {
                    self.renderer.begin_frame();
                    self.video.draw(&self.renderer);
                } else {
                    psx_rt::interrupts::wait_vblank();
                }
            }
            GameState::AttractDemo { ref mut act, ref mut timer } => {
                // Any button press returns to title (only if controller is connected)
                let any_button = is_connected && buttons.bits() != 0;
                if any_button {
                    self.load_act(Act::Act1_1Drainage);
                    self.state = GameState::Title;
                    self.idle_timer = 0;
                } else {
                    *timer += 1;

                    // Simulated demo inputs for each act (gameplay only, no CODEC)
                    // DOWN = North (-Z), UP = South (+Z) [controls now inverted]
                    let sim_buttons = match *act {
                        Act::Act1_1Drainage | Act::Act1_2Barracks => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::UP)
                            } else if *timer < 100 {
                                ButtonState::from_bits(button::LEFT)
                            } else {
                                ButtonState::from_bits(button::CIRCLE | button::UP)
                            }
                        }
                        Act::Act1_3MechBoss | Act::Act2_3JetSkiBoss | Act::Act3_3SniperBoss | Act::Act4_3ExcavatorBoss => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::DOWN)
                            } else if *timer < 100 {
                                ButtonState::from_bits(button::LEFT)
                            } else {
                                ButtonState::from_bits(button::CIRCLE | button::DOWN)
                            }
                        }
                        Act::Act2_1Rapids | Act::Act2_2Mangroves => {
                            if *timer < 50 {
                                ButtonState::from_bits(button::RIGHT)
                            } else if *timer < 90 {
                                ButtonState::from_bits(button::CROSS)
                            } else {
                                ButtonState::from_bits(button::LEFT)
                            }
                        }
                        Act::Act3_1Highway => {
                            if (*timer / 25) % 2 == 0 {
                                ButtonState::from_bits(button::DOWN)
                            } else {
                                ButtonState::NONE
                            }
                        }
                        Act::Act3_2Laneways => {
                            if *timer < 60 {
                                ButtonState::from_bits(button::SQUARE | button::DOWN)
                            } else {
                                ButtonState::from_bits(button::DOWN)
                            }
                        }
                        Act::Act4_1Dunes | Act::Act4_2PierTrench => {
                            if (*timer / 30) % 2 == 0 {
                                ButtonState::from_bits(button::CROSS | button::LEFT | button::DOWN)
                            } else {
                                ButtonState::from_bits(button::LEFT | button::DOWN)
                            }
                        }
                        Act::VrSneaking | Act::VrCqc | Act::VrSonar | Act::VrSpeed => {
                            if *timer < 60 {
                                ButtonState::from_bits(button::DOWN)
                            } else {
                                ButtonState::from_bits(button::CIRCLE | button::DOWN)
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
                        self.platty.in_box,
                        self.platty.vx != 0 || self.platty.vz != 0,
                        self.platty.noise_radius,
                        &self.level,
                    );
                    self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

                    // Render 3D Scene & HUD
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_hud(&self.platty, &self.entities, &self.level, self.frame);

                    // Cycle to next demo act every 180 frames (3 seconds)
                    if *timer >= 180 {
                        *timer = 0;
                        match *act {
                            Act::Act1_1Drainage => {
                                *act = Act::Act1_3MechBoss;
                                self.load_act(Act::Act1_3MechBoss);
                            }
                            Act::Act1_3MechBoss => {
                                *act = Act::Act2_1Rapids;
                                self.load_act(Act::Act2_1Rapids);
                            }
                            Act::Act2_1Rapids => {
                                *act = Act::Act3_1Highway;
                                self.load_act(Act::Act3_1Highway);
                            }
                            Act::Act3_1Highway => {
                                *act = Act::Act4_1Dunes;
                                self.load_act(Act::Act4_1Dunes);
                            }
                            Act::Act4_1Dunes => {
                                self.load_act(Act::Act1_1Drainage);
                                self.state = GameState::Title;
                                self.idle_timer = 0;
                            }
                            _ => {
                                self.load_act(Act::Act1_1Drainage);
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
                        // Intro finished, start Act 1.1!
                        self.load_act(Act::Act1_1Drainage);
                        self.codec.start_conversation(ACT1_1_DIALOGUE);
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
                        if self.level.act.is_boss() {
                            self.state = GameState::BossIntroCutscene { act: self.level.act, timer: 0 };
                        } else if matches!(self.level.act, Act::Act1_1Drainage | Act::Act2_1Rapids | Act::Act3_1Highway | Act::Act4_1Dunes) {
                            self.state = GameState::ChapterTitleCard { act: self.level.act, timer: 0 };
                            AudioManager::play_electro();
                        } else {
                            self.state = GameState::Playing;
                            AudioManager::play_jump();
                        }
                    }
                }

                self.renderer.begin_frame();
                self.codec.draw(&self.renderer.font);
            }
            GameState::ChapterTitleCard { act, ref mut timer } => {
                *timer = timer.saturating_add(1);

                let target_x = self.platty.x;
                let target_y = self.platty.y - 280;
                let target_z = self.platty.z - 240;
                self.renderer.cam_x = target_x;
                self.renderer.cam_y = target_y;
                self.renderer.cam_z = target_z;
                self.renderer.screen_shake = 0;
                self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

                let skip = *timer >= 180 || just_cross || just_start;
                if skip {
                    self.state = GameState::Playing;
                    AudioManager::play_jump();
                } else {
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_chapter_title_card(act, *timer);
                }
            }
            GameState::BossIntroCutscene { act, ref mut timer } => {
                *timer = timer.saturating_add(1);

                if *timer == 1 {
                    AudioManager::play_alert();
                }

                // Smooth cinematic camera orbit around boss
                let (bx, bz) = match act {
                    Act::Act1_3MechBoss => (self.entities.boss_mech.x, self.entities.boss_mech.z),
                    Act::Act2_3JetSkiBoss => (self.entities.boss_jetski.x, self.entities.boss_jetski.z),
                    Act::Act3_3SniperBoss => (self.entities.boss_sniper.x, self.entities.boss_sniper.z),
                    Act::Act4_3ExcavatorBoss => (self.entities.boss_excavator.x, self.entities.boss_excavator.z),
                    _ => (self.platty.x, self.platty.z),
                };

                let ang = ((*timer as u16) * 16) & 0x0FFF;
                let sin_v = psx_gte_core::transform::sin_1_3_12(ang) as i32;
                let cos_v = psx_gte_core::transform::cos_1_3_12(ang) as i32;
                self.renderer.cam_x = bx + ((sin_v * 450) >> 12);
                self.renderer.cam_y = -300;
                self.renderer.cam_z = bz + ((cos_v * 450) >> 12);

                let skip = *timer >= 240 || just_cross || just_start;
                if skip {
                    self.renderer.cam_x = self.platty.x;
                    self.renderer.cam_y = self.platty.y - 280;
                    self.renderer.cam_z = self.platty.z - 240;
                    self.renderer.screen_shake = 0;
                    self.state = GameState::Playing;
                    AudioManager::play_alert();
                } else {
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_boss_title_card(act, *timer);
                }
            }
            GameState::Playing => {
                // Check if player presses START to pause or SELECT to open in-game CODEC radio!
                if just_start {
                    self.pause_selection = 0;
                    self.state = GameState::Paused;
                    AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                } else if just_select {
                    self.codec.open_tuner();
                    self.state = GameState::InGameCodec;
                } else {
                    // Update player and 3D entities
                    let is_crawling = self.platty.state == PlayerState::BellyCrawl;
                    let is_sneaking = self.platty.state == PlayerState::Sneaking;
                    let is_submerged = self.platty.state == PlayerState::Submerged;

                    let prev_alert = self.entities.alert_state;
                    self.mission_stats.play_time_frames = self.mission_stats.play_time_frames.saturating_add(1);
                    self.stage_time_frames = self.stage_time_frames.saturating_add(1);

                    self.platty.update(&pad, self.prev_buttons, &self.level, &mut self.entities);
                    self.entities.update(
                        self.level.act,
                        self.platty.x,
                        self.platty.y,
                        self.platty.z,
                        is_crawling,
                        is_sneaking,
                        is_submerged,
                        self.platty.in_box,
                        self.platty.vx != 0 || self.platty.vz != 0,
                        self.platty.noise_radius,
                        &self.level,
                    );

                    let was_alert = matches!(prev_alert, crate::entities::AlertState::Alert(_));
                    let is_alert = matches!(self.entities.alert_state, crate::entities::AlertState::Alert(_));
                    if !was_alert && is_alert {
                        self.mission_stats.alerts_count = self.mission_stats.alerts_count.saturating_add(1);
                        self.stage_alerts = self.stage_alerts.saturating_add(1);
                    }

                    if self.platty.screen_shake > 0 {
                        self.renderer.screen_shake = self.platty.screen_shake as i16;
                        self.platty.screen_shake = 0;
                    }
                    self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

                    // Check Game Over (health 0)
                    if self.platty.health == 0 {
                        self.idle_timer = 0;
                        self.state = GameState::GameOver;
                        AudioManager::play_hit();
                        self.prev_buttons = ButtonState::NONE;
                        return;
                    }

                    // Check exit or goal reached
                    let reached_exit = match self.level.act {
                        Act::Act1_3MechBoss => {
                            // Stage ends immediately when the mech boss is defeated,
                            // consistent with how the other boss stages (Act2/3/4) work.
                            self.entities.boss_mech.is_defeated()
                        }
                        Act::Act2_3JetSkiBoss => {
                            self.entities.boss_jetski.is_defeated()
                                || self.platty.z <= 3 * TILE_SZ
                                || self.level.is_exit_at(self.platty.x, self.platty.z)
                        }
                        Act::Act3_3SniperBoss => self.entities.boss_sniper.is_defeated(),
                        Act::Act4_3ExcavatorBoss => self.entities.boss_excavator.is_defeated(),
                        Act::Act2_1Rapids => self.platty.z <= 3 * TILE_SZ || self.level.is_exit_at(self.platty.x, self.platty.z),
                        Act::Act3_1Highway => self.platty.z <= 3 * TILE_SZ || self.level.is_exit_at(self.platty.x, self.platty.z),
                        _ => self.level.is_exit_at(self.platty.x, self.platty.z),
                    };

                    if reached_exit {
                        if self.level.act.is_vr() {
                            let vr_idx = (self.level.act as u8).saturating_sub(12);
                            self.save_data.vr_cleared |= 1 << vr_idx;
                            // If all 4 VR sims cleared, reward with Tuxedo & Wireframe
                            if (self.save_data.vr_cleared & 0x0F) == 0x0F {
                                self.save_data.tuxedo_unlocked = 1;
                                self.save_data.wireframe_unlocked = 1;
                            }
                            self.memcard.save_to_slot1(&self.save_data);
                            AudioManager::play_fanfare();
                            self.state = GameState::VrMenu;
                        } else {
                            // Automatically save progress & high score to Memory Card
                            // Clamp campaign progression to Act 4-3 (index 11) so VR stages are not loaded as campaign
                            let next_act_idx = if (self.level.act as u8) < 11 {
                                self.level.act as u8 + 1
                            } else {
                                11
                            };
                            self.save_data.unlocked_act = next_act_idx.max(self.save_data.unlocked_act).min(11);
                            self.save_data.highest_score = self.save_data.highest_score.max(self.platty.score);
                            self.save_data.total_yabbies = self.save_data.total_yabbies.saturating_add(self.platty.yabbies_collected as u16);
                            self.memcard.save_to_slot1(&self.save_data);

                            if self.level.act == Act::Act4_3ExcavatorBoss {
                                let time_s = self.mission_stats.play_time_frames / 60;
                                let codename = crate::save::Codename::evaluate(
                                    self.mission_stats.alerts_count,
                                    self.platty.total_damage,
                                    time_s,
                                    self.platty.takedowns,
                                );

                                // Award unlocks
                                self.save_data.tuxedo_unlocked = 1;
                                self.save_data.wireframe_unlocked = 1;
                                if codename == crate::save::Codename::BigPlatypus {
                                    self.save_data.camo_unlocked = 1;
                                }

                                self.save_data.best_time_seconds = self.save_data.best_time_seconds.min(time_s);
                                self.save_data.alerts_count = self.save_data.alerts_count.min(self.mission_stats.alerts_count);

                                let name_bytes = codename.name().as_bytes();
                                for (i, b) in self.save_data.best_codename.iter_mut().enumerate() {
                                    *b = if i < name_bytes.len() { name_bytes[i] } else { b' ' };
                                }

                                self.memcard.save_to_slot1(&self.save_data);
                                self.video.start_outro();
                                self.state = GameState::OutroVideo { codename };
                            } else {
                                self.state = GameState::StageClear;
                                AudioManager::play_fanfare();
                            }
                        }
                    }

                    // Render 3D World & HUD
                    self.renderer.begin_frame();
                    self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                    self.renderer.draw_hud(&self.platty, &self.entities, &self.level, self.frame);

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
                match self.codec.mode {
                    crate::codec::CodecMode::Tuning => {
                        let memcard_ok = matches!(self.memcard.status, crate::save::SaveStatus::Idle | crate::save::SaveStatus::SaveSuccess);
                        if self.codec.handle_tuner_input(buttons, self.prev_buttons, self.level.act, memcard_ok) {
                            self.state = GameState::Playing;
                        }
                    }
                    crate::codec::CodecMode::InCall => {
                        if self.codec.pending_save {
                            self.save_data.unlocked_act = (self.level.act as u8).max(self.save_data.unlocked_act);
                            self.save_data.highest_score = self.save_data.highest_score.max(self.platty.score);
                            self.save_data.total_yabbies = self.save_data.total_yabbies.max(self.platty.yabbies_collected as u16);
                            self.memcard.save_to_slot1(&self.save_data);
                            self.codec.pending_save = false;
                        }

                        if just_cross || just_select || just_start {
                            if self.codec.on_action_button() {
                                self.state = GameState::Playing;
                            }
                        }
                    }
                }

                self.renderer.begin_frame();
                self.codec.draw(&self.renderer.font);
            }
            GameState::StageClear => {
                if just_cross || just_start {
                    if let Some(next_act) = self.level.act.next() {
                        self.save_data.unlocked_act = (next_act as u8).min(11).max(self.save_data.unlocked_act);
                        self.save_data.highest_score = self.save_data.highest_score.max(self.platty.score);
                        self.save_data.total_yabbies = self.save_data.total_yabbies.max(self.platty.yabbies_collected as u16);
                        self.memcard.save_to_slot1(&self.save_data);

                        self.load_act(next_act);
                        let briefing = get_act_dialogue(next_act);
                        self.codec.start_conversation(briefing);
                        self.state = GameState::StageIntroCodec;
                    } else {
                        self.state = GameState::Ending { codename: None };
                    }
                }

                self.renderer.begin_frame();
                let stage_time_s = self.stage_time_frames / 60;
                let stage_takedowns = self.platty.takedowns.saturating_sub(self.stage_start_takedowns);
                self.renderer.draw_stage_clear(
                    self.level.act,
                    self.platty.score,
                    self.platty.yabbies_collected,
                    stage_time_s,
                    self.stage_alerts,
                    stage_takedowns,
                );
            }
            GameState::Ending { codename } => {
                if just_start || just_cross {
                    self.load_act(Act::Act1_1Drainage);
                    self.state = GameState::Title;
                    self.idle_timer = 0;
                }

                self.renderer.begin_frame();
                self.renderer.draw_ending(self.frame, codename);
            }
            GameState::GameOver => {
                // MGS Classic Game Over: "PLATTY? PLATTY? PLATTYYYYY!"
                self.idle_timer = self.idle_timer.saturating_add(1);
                let can_retry = self.idle_timer > 45 && (just_cross || just_start);
                let return_to_title = self.idle_timer > 45 && (just_circle || just_select || buttons.is_held(button::TRIANGLE) && !self.prev_buttons.is_held(button::TRIANGLE));

                if can_retry {
                    // Retry current stage cleanly
                    let act = self.level.act;
                    self.load_act(act);
                    self.idle_timer = 0;
                    self.prev_buttons = ButtonState::NONE;
                    self.state = GameState::Playing;
                } else if return_to_title {
                    // Abort to Title Screen
                    self.load_act(Act::Act1_1Drainage);
                    self.state = GameState::Title;
                    self.idle_timer = 0;
                    self.prev_buttons = ButtonState::NONE;
                }

                self.renderer.begin_frame();
                gpu::draw_rect_flat(0, 0, 320, 240, 16, 4, 4);
                self.renderer.font.draw_text(115, 80, "GAME OVER", (255, 40, 40));
                self.renderer.font.draw_text(60, 110, "BURROW HQ: PLATTY? PLATTYYYY!", (255, 220, 220));
                if self.idle_timer > 45 {
                    self.renderer.font.draw_text(42, 150, "CROSS: RETRY MISSION    TRIANGLE: TITLE", (255, 255, 255));
                }
            }
            GameState::Paused => {
                if just_up {
                    self.pause_selection = if self.pause_selection == 0 { 2 } else { self.pause_selection - 1 };
                    AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                } else if just_down {
                    self.pause_selection = if self.pause_selection >= 2 { 0 } else { self.pause_selection + 1 };
                    AudioManager::play_footstep(crate::audio::SurfaceType::Concrete);
                } else if just_circle {
                    // Quick unpause on CIRCLE
                    self.state = GameState::Playing;
                    AudioManager::play_jump();
                } else if just_cross || just_start {
                    match self.pause_selection {
                        0 => {
                            // Resume mission
                            self.state = GameState::Playing;
                            AudioManager::play_jump();
                        }
                        1 => {
                            // Retry current act
                            let act = self.level.act;
                            self.load_act(act);
                            self.state = GameState::Playing;
                        }
                        _ => {
                            // Abort mission to title screen
                            self.load_act(Act::Act1_1Drainage);
                            self.state = GameState::Title;
                            self.idle_timer = 0;
                        }
                    }
                    self.prev_buttons = buttons;
                    return;
                }

                // Render background 3D Scene + HUD + Pause Tactical Overlay
                self.renderer.begin_frame();
                self.renderer.draw_3d_scene(&self.level, &self.platty, &self.entities, self.frame);
                self.renderer.draw_hud(&self.platty, &self.entities, &self.level, self.frame);
                self.draw_pause_overlay(self.pause_selection);
            }
        }

        self.prev_buttons = buttons;
    }

    fn load_act(&mut self, act: Act) {
        self.level = Level::new(act);
        self.platty.reset_position(self.level.player_start_x, self.level.player_start_z);
        self.entities.load_act(act);
        self.stage_time_frames = 0;
        self.stage_alerts = 0;
        self.stage_start_takedowns = self.platty.takedowns;

        // Snap camera immediately to spawn target without slow drifting
        let target_x = self.platty.x;
        let target_y = self.platty.y - 280;
        let target_z = self.platty.z - 240;
        self.renderer.cam_x = target_x;
        self.renderer.cam_y = target_y;
        self.renderer.cam_z = target_z;
        self.renderer.screen_shake = 0;
        self.renderer.update_camera(self.platty.x, self.platty.y, self.platty.z);

        let track = if act.is_boss() {
            crate::audio::BgmTrack::Boss
        } else if act.is_vr() {
            crate::audio::BgmTrack::Stealth
        } else {
            match act.chapter() {
                1 => crate::audio::BgmTrack::Stealth,
                2 => crate::audio::BgmTrack::River,
                3 => crate::audio::BgmTrack::City,
                4 => crate::audio::BgmTrack::Beach,
                _ => crate::audio::BgmTrack::Stealth,
            }
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

    fn draw_pause_overlay(&self, selection: usize) {
        let box_x: i16 = 40;
        let box_y: i16 = 60;
        let box_w: u16 = 240;
        let box_h: u16 = 120;

        // Dark tactical overlay box
        gpu::draw_rect_flat(box_x, box_y, box_w, box_h, 10, 16, 24);
        // Cyan tactical border
        gpu::draw_rect_flat(box_x, box_y, box_w, 2, 40, 180, 200);
        gpu::draw_rect_flat(box_x, box_y + box_h as i16 - 2, box_w, 2, 40, 180, 200);
        gpu::draw_rect_flat(box_x, box_y, 2, box_h, 40, 180, 200);
        gpu::draw_rect_flat(box_x + box_w as i16 - 2, box_y, 2, box_h, 40, 180, 200);

        // Header tab
        gpu::draw_rect_flat(box_x + 60, box_y - 6, 120, 14, 15, 45, 60);
        self.renderer.font.draw_text(box_x + 78, box_y - 4, "MISSION PAUSED", (255, 230, 80));

        let items = [
            "RESUME OPERATION",
            "RETRY MISSION",
            "ABORT TO TITLE SCREEN",
        ];

        for (i, label) in items.iter().enumerate() {
            let item_y = box_y + 24 + (i as i16 * 22);
            let is_sel = i == selection;

            if is_sel {
                gpu::draw_rect_flat(box_x + 12, item_y - 2, box_w - 24, 18, 20, 60, 80);
                self.renderer.font.draw_text(box_x + 18, item_y + 2, ">", (255, 240, 100));
                self.renderer.font.draw_text(box_x + 32, item_y + 2, label, (255, 255, 255));
            } else {
                self.renderer.font.draw_text(box_x + 32, item_y + 2, label, (140, 170, 190));
            }
        }

        // Footer helper
        self.renderer.font.draw_text(box_x + 24, box_y + box_h as i16 - 18, "CROSS/START: SELECT   O: RESUME", (100, 140, 160));
    }
}
