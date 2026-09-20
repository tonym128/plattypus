//! Tactical CODEC radio communications system for Plattypus.
//! Inspired by Metal Gear Solid, featuring frequency tuning, animated waveform,
//! character portraits, and dialogue typewriter printing.

use crate::audio::AudioManager;
use psx_font::FontAtlas;
use psx_gpu as gpu;

pub const CODEC_FREQ_BURROW: u16 = 14085; // 140.85 MHz

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Speaker {
    BurrowCommand,
    Platty,
    Mom,
    Dad,
}

impl Speaker {
    pub fn name(&self) -> &'static str {
        match self {
            Speaker::BurrowCommand => "BURROW COMMAND",
            Speaker::Platty => "PLATTY",
            Speaker::Mom => "MOM",
            Speaker::Dad => "DAD",
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

pub static ACT1_START_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Platty, you're outside the",
        line2: "Healesville compound gates.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Sentries patrol the area.",
        line2: "Check radar vision cones!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Crawl under air vents and low",
        line2: "lasers with CIRCLE or DOWN.",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Ambush guards from behind with",
        line2: "SQUARE for a stealth takedown!",
    },
];

pub static ACT2_START_DIALOGUE: &[CodecPage] = &[
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
        line2: "Watch out for river tubers,",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "paddle boarders, swimmers, and",
        line2: "koalas chilling in gum trees!",
    },
];

pub static ACT3_START_DIALOGUE: &[CodecPage] = &[
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
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Reach Pier 9 at the far side",
        line2: "to catch the coastal express!",
    },
];

pub static ACT4_START_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::BurrowCommand,
        line1: "Platty! You made it to the",
        line2: "sunny coastline and dunes!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Pip's nesting burrow is on top",
        line2: "of the highest coastal dune!",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Jump across rock platforms",
        line2: "and dunes by pressing CROSS!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Bounce on colorful beach",
        line2: "umbrellas for mega jumps!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Watch out for beach crabs!",
        line2: "Go meet your baby sister Pip!",
    },
];

pub static RADIO_TIPS_DIALOGUE: &[CodecPage] = &[
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Stage 1: Crawl in tall grass",
        line2: "to stay invisible to guards.",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Stage 2: Shift between the 5",
        line2: "river lanes to dodge tubers!",
    },
    CodecPage {
        speaker: Speaker::Dad,
        line1: "Stage 3: Time car gaps carefully",
        line2: "before crossing city lanes.",
    },
    CodecPage {
        speaker: Speaker::Mom,
        line1: "Stage 4: Bounce on beach",
        line2: "umbrellas to reach high dunes!",
    },
];

// -----------------------------------------------------------------------------
// CODEC MANAGER STATE
// -----------------------------------------------------------------------------

pub struct CodecManager {
    pub pages: &'static [CodecPage],
    pub current_page_idx: usize,
    pub text_progress: usize,
    pub is_active: bool,
    pub anim_timer: u16,
    pub call_chime_timer: u8,
}

impl CodecManager {
    pub fn new() -> Self {
        Self {
            pages: INTRO_DIALOGUE,
            current_page_idx: 0,
            text_progress: 0,
            is_active: false,
            anim_timer: 0,
            call_chime_timer: 0,
        }
    }

    pub fn start_conversation(&mut self, pages: &'static [CodecPage]) {
        self.pages = pages;
        self.current_page_idx = 0;
        self.text_progress = 0;
        self.is_active = true;
        self.anim_timer = 0;
        self.call_chime_timer = 24;
        AudioManager::play_codec_chime();
    }

    /// Advance or complete typewriter text. Returns true if conversation just finished.
    pub fn on_action_button(&mut self) -> bool {
        if !self.is_active {
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
                self.is_active = false;
                AudioManager::play_swoosh();
                true
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

        let page = &self.pages[self.current_page_idx];
        let total_chars = page.line1.len() + page.line2.len();
        if self.text_progress < total_chars {
            self.text_progress += 1;
            if self.text_progress % 2 == 0 {
                AudioManager::play_codec_chirp(self.text_progress);
            }
        }
    }

    /// Render full Metal Gear Solid CODEC screen!
    pub fn draw(&self, font: &FontAtlas) {
        if !self.is_active {
            return;
        }

        // Dark digital green background
        gpu::draw_rect_flat(0, 0, 320, 240, 6, 18, 14);

        // Vector scanline grid lines
        for y in (0..240).step_by(8) {
            gpu::draw_rect_flat(0, y, 320, 1, 10, 32, 22);
        }

        // Top Header Bar
        gpu::draw_rect_flat(0, 0, 320, 24, 8, 30, 20);
        gpu::draw_rect_flat(0, 24, 320, 2, 35, 140, 85);
        font.draw_text(16, 8, "TACTICAL RADIO", (120, 255, 160));
        font.draw_text(210, 8, "FREQ  140.85", (220, 255, 180));

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

        // LEFT PORTRAIT: PLATTY (Tactical Sneaking Platypus)
        Self::draw_portrait_frame(20, 35, "PLATTY");
        Self::draw_platty_portrait(24, 45, self.anim_timer);

        // RIGHT PORTRAIT: BURROW COMMAND (Mom & Dad)
        let page = &self.pages[self.current_page_idx];
        let r_name = page.speaker.name();
        Self::draw_portrait_frame(220, 35, r_name);
        Self::draw_burrow_command_portrait(224, 45, self.anim_timer);

        // BOTTOM CODEC DIALOGUE BOX
        let box_x = 18;
        let box_y = 150;
        let box_w = 284;
        let box_h = 76;

        // Double beveled high-tech military frame
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
        // High-tech vector frame
        gpu::draw_rect_flat(x, y, 80, 95, 25, 90, 60);
        gpu::draw_rect_flat(x + 2, y + 2, 76, 91, 8, 25, 18);
        // Corner bracket accents
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
        // Platty in sneaking gear: brown fur, dark duck bill, green bandana!
        let cx = x + 8;
        let cy = y + 8;

        // Green bandana knot & headband
        gpu::draw_rect_flat(cx + 8, cy + 4, 40, 8, 40, 140, 65);
        gpu::draw_rect_flat(cx + 4, cy + 8, 10, 16, 30, 110, 50); // Knot tail fluttering
        if (anim / 12) % 2 == 0 {
            gpu::draw_rect_flat(cx + 2, cy + 16, 8, 12, 30, 110, 50);
        } else {
            gpu::draw_rect_flat(cx, cy + 18, 10, 10, 30, 110, 50);
        }

        // Platypus head
        gpu::draw_rect_flat(cx + 12, cy + 12, 34, 30, 95, 55, 30);
        gpu::draw_rect_flat(cx + 16, cy + 24, 26, 18, 135, 85, 45); // Cheeks

        // Resolute stealth eyes
        gpu::draw_rect_flat(cx + 24, cy + 15, 6, 4, 255, 255, 255);
        gpu::draw_rect_flat(cx + 26, cy + 16, 3, 3, 0, 0, 0);

        // Big dark leathery duck bill
        gpu::draw_rect_flat(cx + 28, cy + 26, 26, 14, 45, 42, 42);
        gpu::draw_rect_flat(cx + 32, cy + 29, 3, 2, 20, 20, 20); // Nostril

        // Talking bill animation when Platty speaks
        let mouth_open = (anim / 6) % 2 == 0;
        if mouth_open {
            gpu::draw_rect_flat(cx + 34, cy + 34, 18, 3, 20, 20, 20);
        }

        // Sneaking suit high tactical collar
        gpu::draw_rect_flat(cx + 10, cy + 42, 40, 26, 35, 55, 45);
        gpu::draw_rect_flat(cx + 14, cy + 46, 32, 18, 25, 40, 32);
        // Radio throat mic
        gpu::draw_rect_flat(cx + 26, cy + 44, 8, 4, 180, 190, 200);
    }

    fn draw_burrow_command_portrait(x: i16, y: i16, anim: u16) {
        // Mom & Dad Platypus at Burrow Command
        let cx = x + 4;
        let cy = y + 8;

        // Dad (tactical radio headset + burrow helmet)
        gpu::draw_rect_flat(cx + 6, cy + 6, 32, 26, 95, 55, 30);
        // Radio headset band & earpiece
        gpu::draw_rect_flat(cx + 4, cy + 4, 36, 4, 60, 65, 75);
        gpu::draw_rect_flat(cx + 34, cy + 10, 6, 12, 190, 160, 60); // Earpiece
        gpu::draw_rect_flat(cx + 24, cy + 22, 14, 3, 140, 145, 155); // Boom mic
        // Eyes
        gpu::draw_rect_flat(cx + 14, cy + 12, 4, 4, 255, 255, 255);
        gpu::draw_rect_flat(cx + 15, cy + 13, 2, 2, 0, 0, 0);
        // Bill
        gpu::draw_rect_flat(cx + 20, cy + 18, 16, 8, 45, 42, 42);

        // Mom (loving expression with glasses, holding Burrow maps)
        gpu::draw_rect_flat(cx + 36, cy + 22, 30, 28, 105, 65, 35);
        // Reading glasses
        gpu::draw_rect_flat(cx + 44, cy + 26, 8, 6, 220, 180, 60);
        gpu::draw_rect_flat(cx + 46, cy + 28, 4, 3, 200, 240, 255);
        // Bill
        gpu::draw_rect_flat(cx + 48, cy + 34, 16, 8, 50, 45, 45);

        // Subtle mouth animation
        if (anim / 8) % 2 == 0 {
            gpu::draw_rect_flat(cx + 24, cy + 23, 8, 2, 20, 20, 20);
        }

        // Burrow Command map monitors in background
        gpu::draw_rect_flat(cx + 6, cy + 54, 60, 18, 15, 40, 30);
        gpu::draw_rect_flat(cx + 10, cy + 58, 20, 10, 40, 180, 100); // Radar screen
        gpu::draw_rect_flat(cx + 38, cy + 58, 24, 10, 220, 160, 60); // Egg monitor
    }
}
