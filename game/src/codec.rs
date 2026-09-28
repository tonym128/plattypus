//! Tactical CODEC radio communications system for Plattypus.
//! Inspired by Metal Gear Solid, featuring frequency tuning, animated waveform,
//! character portraits, and dialogue typewriter printing.

use crate::audio::AudioManager;
use psx_font::FontAtlas;
use psx_gpu as gpu;
use psx_pad::{button, ButtonState};

pub const CODEC_FREQ_BURROW: u16 = 14085; // 140.85 MHz - Burrow Command
pub const CODEC_FREQ_SCRIBE: u16 = 14096; // 140.96 MHz - Scribe Echidna (Field Save Station)
pub const CODEC_FREQ_JACK: u16 = 14112;   // 141.12 MHz - Kooky Jack (Bushland Intel & Secrets)
pub const CODEC_FREQ_WOMBAT: u16 = 14180; // 141.80 MHz - Dr. Wombat (Tactical Gear Specialist)

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Speaker {
    BurrowCommand,
    Platty,
    Mom,
    Dad,
    ScribeEchidna,
    JackKookaburra,
    DrWombat,
}

impl Speaker {
    pub fn name(&self) -> &'static str {
        match self {
            Speaker::BurrowCommand => "BURROW COMMAND",
            Speaker::Platty => "PLATTY",
            Speaker::Mom => "MOM",
            Speaker::Dad => "DAD",
            Speaker::ScribeEchidna => "SCRIBE ECHIDNA",
            Speaker::JackKookaburra => "KOOKY JACK",
            Speaker::DrWombat => "DR. WOMBAT",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CodecPage {
    pub speaker: Speaker,
    pub line1: &'static str,
    pub line2: &'static str,
}

// -----------------------------------------------------------------------------
// STORY CODEC CONVERSATIONS
// -----------------------------------------------------------------------------

pub static INTRO_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty, do you read us? This",
        line2: "is Burrow Command. Respond!",
    },
    CodecPage {
        speaker: Speaker::Platty,
        line1: "Burrow Command? Mom? Dad?!",
        line2: "Why the tactical frequency?",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Platty, darling! A golden egg",
        line2: "was laid in Coastal Burrow!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "You're gonna be a big brother!",
        line2: "Meet your baby sister Pip!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "The sanctuary rangers detected",
        line2: "us and locked down the reserve!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Searchlights & patrol drones",
        line2: "block every single gate.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "You must execute tactical",
        line2: "espionage action to sneak out!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Crawl low beneath cameras,",
        line2: "submerge deep in waterways,",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "And use your electro-bill",
        line2: "to jam sensors and find food!",
    },
    CodecPage {
        speaker: Speaker::Platty,
        line1: "I understand. I have your",
        line2: "letter safe in my satchel.",
    },
    CodecPage {
        speaker: Speaker::Platty,
        line1: "I'm coming home, Mom and Dad.",
        line2: "Commencing infiltration!",
    },
];

pub static ACT1_1_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty, you've infiltrated the",
        line2: "outer drainage canal grounds.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Sentries patrol canal banks.",
        line2: "Check radar vision cones!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Crawl through drainage pipes",
        line2: "with CIRCLE or DOWN.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Ambush guards from behind with",
        line2: "SQUARE for a venom spur strike!",
    },
];

pub static ACT1_2_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Entering the Ranger Barracks.",
        line2: "Red laser tripwires ahead!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Crawl low beneath the lasers!",
        line2: "Touch a beam and alarm rings!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "If sentries turn towards you,",
        line2: "press L1 to hide in your Box!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Reach the perimeter depot hatch",
        line2: "at the end of the barracks yard!",
    },
];

pub static ACT1_3_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty! Stop! Ahead is the",
        line2: "automated Perimeter Walker!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "The Searchlight Mech MK-I.",
        line2: "Its energy shield is up!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Crawl in drainage trenches to",
        line2: "hide from dual searchlights!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Strike all 3 power conduits",
        line2: "to overload its shield matrix!",
    },
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Once shields are down, sneak",
        line2: "behind and hit its heat core!",
    },
    CodecPage {
        speaker: Speaker::Platty,
        line1: "Conduits first, core second.",
        line2: "I'm taking this walker down!",
    },
];

pub static ACT2_1_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Platty! You're surfing down",
        line2: "the wild Yarra River rapids!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "5 river lanes! Shift lanes",
        line2: "with LEFT and RIGHT on D-Pad!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Jump over logs and tiger",
        line2: "snakes by pressing CROSS!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Slide under hanging branches!",
        line2: "Watch out for river tubers!",
    },
];

pub static ACT2_2_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty, you've entered the",
        line2: "murky mangrove cavern maze.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Zero visibility underwater!",
        line2: "Murky silt blocks your eyes.",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Hold TRIANGLE to charge your",
        line2: "Electro-Sonar Pulse to navigate!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Sonar pulses reveal hidden",
        line2: "spikes and cave tunnels!",
    },
];

pub static ACT2_3_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "ALERT! High-speed patrol craft",
        line2: "approaching from behind!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "It's the Ranger Jet Ski!",
        line2: "He drops floating mine barrels!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Weave between the water mines!",
        line2: "Wait for his engine to overheat!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "When his jet ski stalls, rush",
        line2: "in with a venom spur strike!",
    },
];

pub static ACT3_1_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Dad,
        line1: "You've surfaced in downtown",
        line2: "Melbourne! Look at the towers!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "It's rush hour gridlock!",
        line2: "Cars are zooming both ways!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Cross avenues like a tactical",
        line2: "frogger! Dodge yellow taxis,",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "green trams, and semi trucks!",
        line2: "Rest on sidewalks and medians!",
    },
];

pub static ACT3_2_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "You're in the city laneways",
        line2: "and rooftop catwalks.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Patrol drones hover overhead",
        line2: "scanning the cobblestone alleys!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Pavement makes loud footsteps!",
        line2: "Tilt stick softly to sneak!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Or disguise as a trash box!",
        line2: "Climb up to the radio tower!",
    },
];

pub static ACT3_3_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty, take cover! You're on",
        line2: "the broadcast tower roof!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "It's Sniper Kooky—an elite",
        line2: "cyborg kookaburra perched high!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "When his red laser sights aim,",
        line2: "duck behind AC air chillers!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "After firing, his rifle jams!",
        line2: "Rush his perch and strike him!",
    },
];

pub static ACT4_1_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty! You made it to the",
        line2: "sunny coastline and dunes!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Pip's nesting burrow is past",
        line2: "the high coastal sand bluffs!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Bounce on colorful beach",
        line2: "umbrellas for mega jumps!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Watch out for beach crabs!",
        line2: "Reach the timber pier ahead!",
    },
];

pub static ACT4_2_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "You're beneath the timber pier",
        line2: "crossing over Shark Trench!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Great white sharks patrol the",
        line2: "trench waters! Do not swim!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Jump across the wooden pier",
        line2: "platforms and barnacle pylons!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "The nesting burrow entrance is",
        line2: "right at the end of the pier!",
    },
];

pub static ACT4_3_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "EMERGENCY! Dr. Cane Toad is in",
        line2: "a giant industrial excavator!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "He's trying to excavate the",
        line2: "burrow and steal baby Pip's egg!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Dodge his heavy sweeping claw",
        line2: "and toxic slime mortar shots!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Strike all 4 hydraulic engine",
        line2: "valves to shut the dozer down!",
    },
    CodecPage {
        speaker: Speaker::Platty,
        line1: "Hands off my little sister!",
        line2: "This ends right now!",
    },
];

pub static RADIO_TIPS_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Stealth: Crawl low under lasers",
        line2: "and vents with CIRCLE / DOWN.",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Disguise: Press L1 to deploy",
        line2: "the Cardboard Box and freeze.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Sonar: Hold TRIANGLE to charge",
        line2: "electro-location in dark waters.",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "CQC: Sneak behind enemies and",
        line2: "press SQUARE for venom spur!",
    },
];

pub static SCRIBE_SAVE_SUCCESS_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::ScribeEchidna,
        line1: "Platty, this is Scribe Echidna.",
        line2: "Logging your tactical coordinates...",
    },
    CodecPage {
        speaker: Speaker::ScribeEchidna,
        line1: "Mission progress saved to Slot 1!",
        line2: "Even small tracks mark deep paths.",
    },
];

pub static SCRIBE_SAVE_NO_CARD_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::ScribeEchidna,
        line1: "Platty! No Memory Card was found",
        line2: "in PlayStation Slot 1!",
    },
    CodecPage {
        speaker: Speaker::ScribeEchidna,
        line1: "Insert a Memory Card to secure",
        line2: "your mission progress, mate.",
    },
];

pub static JACK_CH1_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Kookaburra Jack on the horn!",
        line2: "Sanctuary guards are blind fools!",
    },
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Crawl through drainage pipes and",
        line2: "nail them from behind with CQC!",
    },
];

pub static JACK_CH2_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "River's running wild today, Platty!",
        line2: "Hold TRIANGLE to pulse your sonar!",
    },
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Electro-reception lights up",
        line2: "sunken snags and hidden yabbies!",
    },
];

pub static JACK_CH3_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "City concrete echoes your steps!",
        line2: "Guards have sharp ears on stone.",
    },
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Tilt stick softly to sneak,",
        line2: "or duck under a Cardboard Box!",
    },
];

pub static JACK_CH4_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Smell that sea breeze, little mate?",
        line2: "You're right near Coastal Burrow!",
    },
    CodecPage {
        speaker: Speaker::JackKookaburra,
        line1: "Bounce on beach parasols to leap",
        line2: "the high cliffs and reach baby Pip!",
    },
];

pub static WOMBAT_GEAR_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::DrWombat,
        line1: "Dr. Wombat here. How is the",
        line2: "burrow stealth suit holding up?",
    },
    CodecPage {
        speaker: Speaker::DrWombat,
        line1: "Your Cardboard Box is military",
        line2: "grade: freeze motionless to evade!",
    },
    CodecPage {
        speaker: Speaker::DrWombat,
        line1: "And your hind venom spurs deliver",
        line2: "instant neuro-stuns to any sentry!",
    },
];

pub static STATIC_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Platty,
        line1: "... Just white noise static.",
        line2: "No tactical station on this freq.",
    },
];

// Backward-compatibility aliases
pub static ACT1_START_DIALOGUE: &[CodecPage] = ACT1_1_DIALOGUE;
pub static ACT1_BOSS_DIALOGUE: &[CodecPage] = ACT1_3_DIALOGUE;
pub static ACT2_START_DIALOGUE: &[CodecPage] = ACT2_1_DIALOGUE;
pub static ACT3_START_DIALOGUE: &[CodecPage] = ACT3_1_DIALOGUE;
pub static ACT4_START_DIALOGUE: &[CodecPage] = ACT4_1_DIALOGUE;

pub fn get_act_dialogue(act: crate::level::Act) -> &'static [CodecPage] {
    match act {
        crate::level::Act::Act1_1Drainage => ACT1_1_DIALOGUE,
        crate::level::Act::Act1_2Barracks => ACT1_2_DIALOGUE,
        crate::level::Act::Act1_3MechBoss => ACT1_3_DIALOGUE,
        crate::level::Act::Act2_1Rapids => ACT2_1_DIALOGUE,
        crate::level::Act::Act2_2Mangroves => ACT2_2_DIALOGUE,
        crate::level::Act::Act2_3JetSkiBoss => ACT2_3_DIALOGUE,
        crate::level::Act::Act3_1Highway => ACT3_1_DIALOGUE,
        crate::level::Act::Act3_2Laneways => ACT3_2_DIALOGUE,
        crate::level::Act::Act3_3SniperBoss => ACT3_3_DIALOGUE,
        crate::level::Act::Act4_1Dunes => ACT4_1_DIALOGUE,
        crate::level::Act::Act4_2PierTrench => ACT4_2_DIALOGUE,
        crate::level::Act::Act4_3ExcavatorBoss => ACT4_3_DIALOGUE,
        crate::level::Act::VrSneaking
        | crate::level::Act::VrCqc
        | crate::level::Act::VrSonar
        | crate::level::Act::VrSpeed => ACT1_1_DIALOGUE,
    }
}

// -----------------------------------------------------------------------------
// CODEC MANAGER STATE & FREQUENCY TUNER
// -----------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CodecMode {
    Tuning,
    InCall,
}

pub struct CodecManager {
    pub mode: CodecMode,
    pub tuned_freq: u16,
    pub pages: &'static [CodecPage],
    pub current_page_idx: usize,
    pub text_progress: usize,
    pub is_active: bool,
    pub anim_timer: u16,
    pub call_chime_timer: u8,
    pub is_story_cutscene: bool,
    pub pending_save: bool,
}

impl CodecManager {
    pub fn new() -> Self {
        Self {
            mode: CodecMode::InCall,
            tuned_freq: CODEC_FREQ_BURROW,
            pages: INTRO_DIALOGUE,
            current_page_idx: 0,
            text_progress: 0,
            is_active: false,
            anim_timer: 0,
            call_chime_timer: 0,
            is_story_cutscene: false,
            pending_save: false,
        }
    }

    /// Open radio tuner screen during active gameplay (SELECT button)
    pub fn open_tuner(&mut self) {
        self.mode = CodecMode::Tuning;
        self.is_active = true;
        self.anim_timer = 0;
        self.is_story_cutscene = false;
        self.pending_save = false;
        AudioManager::play_electro();
    }

    /// Start a direct story transmission without tuning mode (e.g. Act intro / Stage clear)
    pub fn start_conversation(&mut self, pages: &'static [CodecPage]) {
        self.pages = pages;
        self.current_page_idx = 0;
        self.text_progress = 0;
        self.mode = CodecMode::InCall;
        self.is_active = true;
        self.anim_timer = 0;
        self.call_chime_timer = 24;
        self.is_story_cutscene = true;
        self.pending_save = false;
        AudioManager::play_codec_chime();
    }

    /// Connect call to the currently tuned frequency
    pub fn call_current_frequency(&mut self, act: crate::level::Act, memcard_ok: bool) {
        let pages = match self.tuned_freq {
            CODEC_FREQ_BURROW => get_act_dialogue(act),
            CODEC_FREQ_SCRIBE => {
                self.pending_save = true;
                if memcard_ok {
                    SCRIBE_SAVE_SUCCESS_DIALOGUE
                } else {
                    SCRIBE_SAVE_NO_CARD_DIALOGUE
                }
            }
            CODEC_FREQ_JACK => match act.chapter() {
                1 => JACK_CH1_DIALOGUE,
                2 => JACK_CH2_DIALOGUE,
                3 => JACK_CH3_DIALOGUE,
                _ => JACK_CH4_DIALOGUE,
            },
            CODEC_FREQ_WOMBAT => WOMBAT_GEAR_DIALOGUE,
            _ => STATIC_DIALOGUE,
        };
        self.pages = pages;
        self.current_page_idx = 0;
        self.text_progress = 0;
        self.mode = CodecMode::InCall;
        self.anim_timer = 0;
        self.call_chime_timer = 24;
        AudioManager::play_codec_chime();
    }

    /// Handle controller inputs when tuner dial is active.
    /// Returns true if player pressed CIRCLE or SELECT to exit the radio.
    pub fn handle_tuner_input(
        &mut self,
        buttons: ButtonState,
        prev_buttons: ButtonState,
        act: crate::level::Act,
        memcard_ok: bool,
    ) -> bool {
        let just_left = buttons.is_held(button::LEFT) && !prev_buttons.is_held(button::LEFT);
        let just_right = buttons.is_held(button::RIGHT) && !prev_buttons.is_held(button::RIGHT);
        let just_down = buttons.is_held(button::DOWN) && !prev_buttons.is_held(button::DOWN);
        let just_cross = buttons.is_held(button::CROSS) && !prev_buttons.is_held(button::CROSS);
        let just_up = buttons.is_held(button::UP) && !prev_buttons.is_held(button::UP);
        let just_circle = buttons.is_held(button::CIRCLE) && !prev_buttons.is_held(button::CIRCLE);
        let just_select = buttons.is_held(button::SELECT) && !prev_buttons.is_held(button::SELECT);

        if just_circle || just_select {
            self.is_active = false;
            AudioManager::play_swoosh();
            return true;
        }

        if just_left {
            self.tuned_freq = self.tuned_freq.saturating_sub(1).max(14000);
            AudioManager::play_electro();
        } else if just_right {
            self.tuned_freq = self.tuned_freq.saturating_add(1).min(14250);
            AudioManager::play_electro();
        } else if just_down {
            // Cycle presets: 140.85 -> 140.96 -> 141.12 -> 141.80 -> 140.85
            self.tuned_freq = match self.tuned_freq {
                CODEC_FREQ_BURROW => CODEC_FREQ_SCRIBE,
                CODEC_FREQ_SCRIBE => CODEC_FREQ_JACK,
                CODEC_FREQ_JACK => CODEC_FREQ_WOMBAT,
                _ => CODEC_FREQ_BURROW,
            };
            AudioManager::play_electro();
        }

        if just_cross || just_up {
            self.call_current_frequency(act, memcard_ok);
        }

        false
    }

    /// Advance or complete typewriter text during a call. Returns true if conversation just finished.
    pub fn on_action_button(&mut self) -> bool {
        if !self.is_active || self.mode != CodecMode::InCall {
            return false;
        }
        let page = &self.pages[self.current_page_idx];
        let total_chars = page.line1.len() + page.line2.len();

        if self.text_progress < total_chars {
            // Fast-forward to end of current page
            self.text_progress = total_chars;
            false
        } else {
            // Advance to next page
            self.current_page_idx += 1;
            self.text_progress = 0;
            if self.current_page_idx >= self.pages.len() {
                if self.is_story_cutscene {
                    self.is_active = false;
                    AudioManager::play_swoosh();
                    true
                } else {
                    // Return to frequency tuner
                    self.mode = CodecMode::Tuning;
                    AudioManager::play_swoosh();
                    false
                }
            } else {
                AudioManager::play_electro();
                false
            }
        }
    }

    pub fn update(&mut self) {
        if !self.is_active {
            return;
        }
        self.anim_timer = self.anim_timer.wrapping_add(1);

        if self.mode == CodecMode::InCall {
            let page = &self.pages[self.current_page_idx];
            let total_chars = page.line1.len() + page.line2.len();
            if self.text_progress < total_chars {
                self.text_progress += 1;
                if self.text_progress % 2 == 0 {
                    AudioManager::play_codec_chirp(self.text_progress);
                }
            }
        }
    }

    /// Render full Metal Gear Solid CODEC screen (Tuning or InCall)!
    pub fn draw(&self, font: &FontAtlas) {
        if !self.is_active {
            return;
        }

        match self.mode {
            CodecMode::Tuning => self.draw_tuner(font),
            CodecMode::InCall => self.draw_incall(font),
        }
    }

    fn draw_tuner(&self, font: &FontAtlas) {
        // Dark digital green background
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 18, 14);

        // Scanline grid
        for y in (0..240).step_by(8) {
            gpu::draw_rect_flat(0, y, 320, 1, 10, 32, 22);
        }

        // Top Header Bar
        gpu::draw_rect_flat(0, 0, 320, 24, 8, 30, 20);
        gpu::draw_rect_flat(0, 24, 320, 2, 35, 140, 85);
        font.draw_text(16, 8, "TACTICAL RADIO TUNER", (120, 255, 160));
        font.draw_text(220, 8, "SELECT: EXIT", (200, 200, 200));

        // Tuner Dial Frame in Center (x: 40, y: 36, w: 240, h: 64)
        gpu::draw_rect_flat(40, 36, 240, 64, 12, 45, 30);
        gpu::draw_rect_flat(42, 38, 236, 60, 4, 18, 12);

        // Digital Frequency Display
        let whole = self.tuned_freq / 100;
        let frac = self.tuned_freq % 100;
        let mut f_str = [b'F', b'R', b'E', b'Q', b' ', b'0', b'0', b'0', b'.', b'0', b'0', b' ', b'M', b'H', b'z'];
        f_str[5] = (whole / 100) as u8 + b'0';
        f_str[6] = ((whole / 10) % 10) as u8 + b'0';
        f_str[7] = (whole % 10) as u8 + b'0';
        f_str[9] = (frac / 10) as u8 + b'0';
        f_str[10] = (frac % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&f_str) {
            font.draw_text(100, 48, s, (100, 255, 140));
        }

        // Frequency Slider Bar
        gpu::draw_rect_flat(50, 72, 220, 10, 8, 28, 20);
        for i in 0..11 {
            let tx = 52 + i * 21;
            gpu::draw_rect_flat(tx, 74, 1, 6, 40, 120, 80);
        }
        let needle_x = 52 + (((self.tuned_freq.saturating_sub(14000) as u32) * 214) / 250) as i16;
        gpu::draw_rect_flat(needle_x - 2, 69, 5, 16, 255, 60, 60);

        // Contact Directory / Presets Frame (x: 40, y: 110, w: 240, h: 86)
        gpu::draw_rect_flat(40, 110, 240, 86, 12, 45, 30);
        gpu::draw_rect_flat(42, 112, 236, 82, 4, 18, 12);
        font.draw_text(48, 116, "RADIO DIRECTORY PRESETS:", (255, 230, 80));

        let is_burrow = self.tuned_freq == CODEC_FREQ_BURROW;
        let is_scribe = self.tuned_freq == CODEC_FREQ_SCRIBE;
        let is_jack = self.tuned_freq == CODEC_FREQ_JACK;
        let is_wombat = self.tuned_freq == CODEC_FREQ_WOMBAT;

        let col_b = if is_burrow { (100, 255, 160) } else { (140, 170, 150) };
        let col_s = if is_scribe { (100, 255, 160) } else { (140, 170, 150) };
        let col_j = if is_jack { (100, 255, 160) } else { (140, 170, 150) };
        let col_w = if is_wombat { (100, 255, 160) } else { (140, 170, 150) };

        font.draw_text(52, 130, if is_burrow { "> 140.85  BURROW COMMAND" } else { "  140.85  BURROW COMMAND" }, col_b);
        font.draw_text(52, 144, if is_scribe { "> 140.96  SCRIBE ECHIDNA (SAVE)" } else { "  140.96  SCRIBE ECHIDNA (SAVE)" }, col_s);
        font.draw_text(52, 158, if is_jack { "> 141.12  KOOKY JACK (INTEL)" } else { "  141.12  KOOKY JACK (INTEL)" }, col_j);
        font.draw_text(52, 172, if is_wombat { "> 141.80  DR. WOMBAT (GEAR)" } else { "  141.80  DR. WOMBAT (GEAR)" }, col_w);

        // Control instructions bar
        gpu::draw_rect_flat(0, 204, 320, 36, 8, 30, 20);
        gpu::draw_rect_flat(0, 204, 320, 2, 35, 140, 85);
        font.draw_text(18, 210, "D-PAD L/R: TUNE   DOWN: CYCLE PRESET", (220, 240, 220));
        font.draw_text(18, 222, "CROSS / UP: TRANSMIT   CIRCLE: EXIT", (255, 230, 80));
    }

    fn draw_incall(&self, font: &FontAtlas) {
        // Dark digital green background
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 18, 14);

        // Scanline grid
        for y in (0..240).step_by(8) {
            gpu::draw_rect_flat(0, y, 320, 1, 10, 32, 22);
        }

        // Top Header Bar
        gpu::draw_rect_flat(0, 0, 320, 24, 8, 30, 20);
        gpu::draw_rect_flat(0, 24, 320, 2, 35, 140, 85);
        font.draw_text(16, 8, "TACTICAL RADIO", (120, 255, 160));

        let whole = self.tuned_freq / 100;
        let frac = self.tuned_freq % 100;
        let mut f_hdr = [b'F', b'R', b'E', b'Q', b' ', b' ', b'0', b'0', b'0', b'.', b'0', b'0'];
        f_hdr[6] = (whole / 100) as u8 + b'0';
        f_hdr[7] = ((whole / 10) % 10) as u8 + b'0';
        f_hdr[8] = (whole % 10) as u8 + b'0';
        f_hdr[10] = (frac / 10) as u8 + b'0';
        f_hdr[11] = (frac % 10) as u8 + b'0';
        if let Ok(s) = core::str::from_utf8(&f_hdr) {
            font.draw_text(205, 8, s, (220, 255, 180));
        }

        // Oscillating green soundwave bars in center
        let wave_y = 65;
        gpu::draw_rect_flat(120, 45, 80, 40, 10, 35, 25);
        gpu::draw_rect_flat(122, 47, 76, 36, 4, 14, 10);
        for bar in 0..11 {
            let bx = 126 + bar * 6;
            let h = ((self.anim_timer as i16 * 4 + bar * 23) % 24).abs().max(4);
            gpu::draw_rect_flat(bx, wave_y - h / 2, 4, h as u16, 50, 240, 110);
        }
        font.draw_text(130, 92, "MEMORY", (90, 190, 120));

        // LEFT PORTRAIT: PLATTY
        Self::draw_portrait_frame(20, 35, "PLATTY");
        Self::draw_platty_portrait(24, 45, self.anim_timer);

        // RIGHT PORTRAIT: CONTACT BASED ON SPEAKER
        let page = &self.pages[self.current_page_idx];
        let r_name = page.speaker.name();
        Self::draw_portrait_frame(220, 35, r_name);
        match page.speaker {
            Speaker::BurrowCommand | Speaker::Mom | Speaker::Dad => {
                Self::draw_burrow_command_portrait(224, 45, self.anim_timer);
            }
            Speaker::ScribeEchidna => {
                Self::draw_scribe_portrait(224, 45, self.anim_timer);
            }
            Speaker::JackKookaburra => {
                Self::draw_kookaburra_portrait(224, 45, self.anim_timer);
            }
            Speaker::DrWombat => {
                Self::draw_wombat_portrait(224, 45, self.anim_timer);
            }
            Speaker::Platty => {
                Self::draw_static_portrait(224, 45, self.anim_timer);
            }
        }

        // BOTTOM CODEC DIALOGUE BOX
        let box_x = 18;
        let box_y = 150;
        let box_w = 284;
        let box_h = 76;

        gpu::draw_rect_flat(box_x, box_y, box_w, box_h, 12, 45, 30);
        gpu::draw_rect_flat(box_x + 2, box_y + 2, box_w - 4, box_h - 4, 6, 22, 16);
        gpu::draw_rect_flat(box_x + 4, box_y + 4, box_w - 8, box_h - 8, 10, 35, 24);

        // Speaker tag banner
        gpu::draw_rect_flat(box_x + 8, box_y - 6, 120, 12, 15, 65, 45);
        font.draw_text(box_x + 12, box_y - 4, page.speaker.name(), (255, 240, 120));

        // Render typewriter text for Line 1 and Line 2
        let l1_len = page.line1.len();
        let l1_chars = self.text_progress.min(l1_len);
        if l1_chars > 0 {
            if let Some(sub1) = page.line1.get(..l1_chars) {
                font.draw_text(box_x + 12, box_y + 18, sub1, (240, 255, 240));
            }
        }

        if self.text_progress > l1_len {
            let l2_chars = (self.text_progress - l1_len).min(page.line2.len());
            if l2_chars > 0 {
                if let Some(sub2) = page.line2.get(..l2_chars) {
                    font.draw_text(box_x + 12, box_y + 36, sub2, (240, 255, 240));
                }
            }
        }

        // Flashing page advance indicator
        let total_chars = page.line1.len() + page.line2.len();
        if self.text_progress >= total_chars {
            if (self.anim_timer / 15) % 2 == 0 {
                font.draw_text(box_x + box_w as i16 - 24, box_y + box_h as i16 - 16, ">", (255, 230, 80));
            }
        }
    }

    fn draw_portrait_frame(x: i16, y: i16, _name: &str) {
        gpu::draw_rect_flat(x, y, 80, 95, 25, 90, 60);
        gpu::draw_rect_flat(x + 2, y + 2, 76, 91, 8, 25, 18);
        gpu::draw_rect_flat(x, y, 8, 2, 80, 255, 140);
        gpu::draw_rect_flat(x, y, 2, 8, 80, 255, 140);
        gpu::draw_rect_flat(x + 72, y, 8, 2, 80, 255, 140);
        gpu::draw_rect_flat(x + 78, y, 2, 8, 80, 255, 140);
        gpu::draw_rect_flat(x, y + 93, 8, 2, 80, 255, 140);
        gpu::draw_rect_flat(x, y + 87, 2, 8, 80, 255, 140);
        gpu::draw_rect_flat(x + 72, y + 93, 8, 2, 80, 255, 140);
        gpu::draw_rect_flat(x + 78, y + 87, 2, 8, 80, 255, 140);
    }

    fn draw_platty_portrait(x: i16, y: i16, anim: u16) {
        let cx = x + 8;
        let cy = y + 8;

        gpu::draw_rect_flat(cx + 8, cy + 4, 40, 8, 40, 140, 65);
        gpu::draw_rect_flat(cx + 4, cy + 8, 10, 16, 30, 110, 50);
        if (anim / 12) % 2 == 0 {
            gpu::draw_rect_flat(cx + 2, cy + 16, 8, 12, 30, 110, 50);
        } else {
            gpu::draw_rect_flat(cx, cy + 18, 10, 10, 30, 110, 50);
        }

        gpu::draw_rect_flat(cx + 12, cy + 12, 34, 30, 95, 55, 30);
        gpu::draw_rect_flat(cx + 16, cy + 24, 26, 18, 135, 85, 45);

        gpu::draw_rect_flat(cx + 24, cy + 15, 6, 4, 255, 255, 255);
        gpu::draw_rect_flat(cx + 26, cy + 16, 3, 3, 0, 0, 0);

        gpu::draw_rect_flat(cx + 28, cy + 26, 26, 14, 45, 42, 42);
        gpu::draw_rect_flat(cx + 32, cy + 29, 3, 2, 20, 20, 20);

        let mouth_open = (anim / 6) % 2 == 0;
        if mouth_open {
            gpu::draw_rect_flat(cx + 34, cy + 34, 18, 3, 20, 20, 20);
        }

        gpu::draw_rect_flat(cx + 10, cy + 42, 40, 26, 35, 55, 45);
        gpu::draw_rect_flat(cx + 14, cy + 46, 32, 18, 25, 40, 32);
        gpu::draw_rect_flat(cx + 26, cy + 44, 8, 4, 180, 190, 200);
    }

    fn draw_burrow_command_portrait(x: i16, y: i16, anim: u16) {
        let cx = x + 4;
        let cy = y + 8;

        gpu::draw_rect_flat(cx + 6, cy + 6, 32, 26, 95, 55, 30);
        gpu::draw_rect_flat(cx + 4, cy + 4, 36, 4, 60, 65, 75);
        gpu::draw_rect_flat(cx + 34, cy + 10, 6, 12, 190, 160, 60);
        gpu::draw_rect_flat(cx + 24, cy + 22, 14, 3, 140, 145, 155);
        gpu::draw_rect_flat(cx + 14, cy + 12, 4, 4, 255, 255, 255);
        gpu::draw_rect_flat(cx + 15, cy + 13, 2, 2, 0, 0, 0);
        gpu::draw_rect_flat(cx + 20, cy + 18, 16, 8, 45, 42, 42);

        gpu::draw_rect_flat(cx + 36, cy + 22, 30, 28, 105, 65, 35);
        gpu::draw_rect_flat(cx + 44, cy + 26, 8, 6, 220, 180, 60);
        gpu::draw_rect_flat(cx + 46, cy + 28, 4, 3, 200, 240, 255);
        gpu::draw_rect_flat(cx + 48, cy + 34, 16, 8, 50, 45, 45);

        if (anim / 8) % 2 == 0 {
            gpu::draw_rect_flat(cx + 24, cy + 23, 8, 2, 20, 20, 20);
        }

        gpu::draw_rect_flat(cx + 6, cy + 54, 60, 18, 15, 40, 30);
        gpu::draw_rect_flat(cx + 10, cy + 58, 20, 10, 40, 180, 100);
        gpu::draw_rect_flat(cx + 38, cy + 58, 24, 10, 220, 160, 60);
    }

    fn draw_scribe_portrait(x: i16, y: i16, anim: u16) {
        let cx = x + 8;
        let cy = y + 8;

        // Echidna spines / quills crest
        for i in 0..5 {
            let sx = cx + 8 + i * 8;
            let sy = cy + 4 - ((i - 2) * (i - 2) * 2);
            gpu::draw_rect_flat(sx, sy, 5, 14, 210, 180, 120);
        }

        // Head and snout
        gpu::draw_rect_flat(cx + 12, cy + 16, 36, 28, 140, 100, 60);
        // Long slender tubular echidna snout
        gpu::draw_rect_flat(cx + 40, cy + 26, 22, 7, 100, 70, 45);
        gpu::draw_rect_flat(cx + 60, cy + 28, 3, 3, 20, 20, 20);

        // Round scholar spectacles
        gpu::draw_rect_flat(cx + 26, cy + 20, 10, 10, 240, 210, 60);
        gpu::draw_rect_flat(cx + 28, cy + 22, 6, 6, 200, 240, 255);
        gpu::draw_rect_flat(cx + 31, cy + 24, 2, 2, 20, 20, 20); // Eye pupil

        // Scholar quill pen in paw
        let bob = if (anim / 10) % 2 == 0 { 2 } else { 0 };
        gpu::draw_rect_flat(cx + 14, cy + 48 + bob, 14, 10, 140, 100, 60); // Paw
        gpu::draw_rect_flat(cx + 22, cy + 40 + bob, 3, 16, 255, 255, 240); // White feather quill
        gpu::draw_rect_flat(cx + 23, cy + 56 + bob, 2, 4, 30, 30, 40);   // Ink tip
    }

    fn draw_kookaburra_portrait(x: i16, y: i16, anim: u16) {
        let cx = x + 8;
        let cy = y + 8;

        // Kookaburra head & crest (white with brown crown)
        gpu::draw_rect_flat(cx + 10, cy + 8, 38, 10, 100, 65, 40); // Brown crown
        gpu::draw_rect_flat(cx + 10, cy + 18, 38, 24, 240, 235, 230); // White head

        // Distinct brown eye stripe
        gpu::draw_rect_flat(cx + 18, cy + 20, 26, 6, 90, 55, 35);
        // Sharp hunting eye
        gpu::draw_rect_flat(cx + 28, cy + 21, 5, 4, 255, 240, 80);
        gpu::draw_rect_flat(cx + 30, cy + 22, 2, 2, 0, 0, 0);

        // Heavy stout dagger beak
        gpu::draw_rect_flat(cx + 38, cy + 24, 26, 8, 55, 50, 48); // Upper black beak
        gpu::draw_rect_flat(cx + 38, cy + 32, 24, 6, 210, 190, 160); // Lower bone beak

        // Aviator goggles pushed up on crown
        gpu::draw_rect_flat(cx + 16, cy + 12, 12, 7, 180, 130, 40);
        gpu::draw_rect_flat(cx + 30, cy + 12, 12, 7, 180, 130, 40);
        gpu::draw_rect_flat(cx + 18, cy + 14, 8, 3, 160, 220, 255);
        gpu::draw_rect_flat(cx + 32, cy + 14, 8, 3, 160, 220, 255);

        // Blue shoulder flash plumage
        gpu::draw_rect_flat(cx + 8, cy + 44, 44, 24, 70, 130, 190);
        if (anim / 8) % 2 == 0 {
            gpu::draw_rect_flat(cx + 42, cy + 30, 18, 3, 20, 20, 20); // Laughing beak open
        }
    }

    fn draw_wombat_portrait(x: i16, y: i16, _anim: u16) {
        let cx = x + 8;
        let cy = y + 8;

        // Round furry wombat ears
        gpu::draw_rect_flat(cx + 8, cy + 6, 12, 12, 90, 85, 80);
        gpu::draw_rect_flat(cx + 40, cy + 6, 12, 12, 90, 85, 80);

        // Broad wombat head
        gpu::draw_rect_flat(cx + 10, cy + 14, 42, 34, 110, 105, 100);

        // Little dark bead eyes
        gpu::draw_rect_flat(cx + 18, cy + 22, 4, 4, 20, 20, 20);
        gpu::draw_rect_flat(cx + 38, cy + 22, 4, 4, 20, 20, 20);

        // Large rounded leathery wombat nose
        gpu::draw_rect_flat(cx + 24, cy + 28, 14, 12, 60, 50, 50);
        gpu::draw_rect_flat(cx + 26, cy + 34, 3, 3, 20, 20, 20);
        gpu::draw_rect_flat(cx + 33, cy + 34, 3, 3, 20, 20, 20);

        // Lab coat & tactical stethoscope
        gpu::draw_rect_flat(cx + 10, cy + 46, 42, 22, 230, 235, 240);
        gpu::draw_rect_flat(cx + 24, cy + 48, 14, 12, 70, 75, 85);
    }

    fn draw_static_portrait(x: i16, y: i16, anim: u16) {
        let cx = x + 8;
        let cy = y + 8;

        // Animated TV static noise
        for row in 0..12 {
            for col in 0..11 {
                let seed = (anim as usize * 37 + row * 19 + col * 13) % 7;
                let c = match seed {
                    0 | 1 => 40,
                    2 | 3 => 140,
                    _ => 220,
                };
                gpu::draw_rect_flat(cx + (col as i16 * 4), cy + (row as i16 * 5), 4, 5, c, c, c);
            }
        }
    }
}
