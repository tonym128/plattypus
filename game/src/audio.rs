//! Hardware SPU audio engine, dynamic multi-channel music sequencer,
//! tactical stealth sound effects, and CODEC radio synthesizers for Plattypus MGS.

use psx_asset::Audio;
use psx_io::cdrom;
use psx_spu::{self as spu, tones, Adsr, Pitch, SpuAddr, Voice, Volume};

static JUMP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/jump.psau");
static COIN_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/pickup_coin.psau");
static SWOOSH_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/swoosh.psau");
static PUNCH_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/hit_punch.psau");
static METAL_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/hit_metal.psau");
static BEEP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/ui_beep.psau");
static FOOTSTEP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/footstep.psau");
static SELECT_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/ui_select.psau");

static TITLE_MUSIC_ADPCM: &[u8] = include_bytes!("../../Music/title_music.adpcm");
static INTRO_AUDIO_VAG: &[u8] = include_bytes!("../../Videos/intro_audio.vag");
static OUTRO_AUDIO_VAG: &[u8] = include_bytes!("../../Videos/outro_audio.vag");

const SPU_SAMPLE_BASE: u32 = 0x1010;

// SFX Voices (0..12)
pub const VOICE_JUMP: Voice = Voice::V0;
pub const VOICE_YABBY: Voice = Voice::V1;
pub const VOICE_SWOOSH: Voice = Voice::V2;
pub const VOICE_HIT: Voice = Voice::V3;
pub const VOICE_METAL: Voice = Voice::V4;
pub const VOICE_ELECTRO: Voice = Voice::V5;
pub const VOICE_FOOTSTEP: Voice = Voice::V6;
pub const VOICE_SPLASH: Voice = Voice::V7;
pub const VOICE_ALERT: Voice = Voice::new(8);
pub const VOICE_SPUR: Voice = Voice::new(9);
pub const VOICE_CHIME: Voice = Voice::new(10);
pub const VOICE_VOICE: Voice = Voice::new(11);
pub const VOICE_SELECT: Voice = Voice::new(12);

// Custom Title Music Voice & Cinematic Video Voices
pub const VOICE_TITLE: Voice = Voice::new(13);
pub const VOICE_INTRO: Voice = Voice::new(14);
pub const VOICE_OUTRO: Voice = Voice::new(15);

// Music Synthesizer Voices (16..19)
pub const VOICE_BASS: Voice = Voice::new(16);
pub const VOICE_LEAD: Voice = Voice::new(17);
pub const VOICE_HARMONY: Voice = Voice::new(18);
pub const VOICE_DRUM: Voice = Voice::new(19);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SurfaceType {
    Concrete,
    Metal,
    Grass,
    Water,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BgmTrack {
    None,
    Title,
    Stealth,
    Alert,
    River,
    City,
    Beach,
    Boss,
}

pub struct AudioManager {
    current_track: BgmTrack,
    seq_step: u8,
    tempo_counter: u8,
    tempo_period: u8,
    chime_timer: u8,
}

static mut AUDIO_STATE: AudioManager = AudioManager {
    current_track: BgmTrack::None,
    seq_step: 0,
    tempo_counter: 0,
    tempo_period: 8,
    chime_timer: 0,
};

static mut ADDR_TRIANGLE: SpuAddr = SpuAddr::new(0x1000);
static mut ADDR_SAWTOOTH: SpuAddr = SpuAddr::new(0x1000);
static mut ADDR_SQUARE: SpuAddr = SpuAddr::new(0x1000);
static mut ADDR_SINE: SpuAddr = SpuAddr::new(0x1000);
static mut ADDR_TITLE_MUSIC: SpuAddr = SpuAddr::new(0x1000);
static mut TITLE_MUSIC_PLAYING: bool = false;
static mut CDDA_WAS_PLAYING: bool = false;

impl AudioManager {
    pub fn init() {
        spu::init();
        spu::set_main_volume(Volume::MAX, Volume::MAX);
        spu::set_cd_volume(spu::CdVolume::MAX, spu::CdVolume::MAX);
        spu::enable_cd_audio(true);

        // CD-DA mode only (no double-speed for audio playback)
        cdrom::set_mode(cdrom::MODE_CDDA);
        cdrom::demute();

        let sfx = [
            (VOICE_JUMP, JUMP_SFX, Volume::linear(1, 4)),
            (VOICE_YABBY, COIN_SFX, Volume::linear(1, 4)),
            (VOICE_SWOOSH, SWOOSH_SFX, Volume::linear(1, 3)),
            (VOICE_HIT, PUNCH_SFX, Volume::linear(1, 3)),
            (VOICE_METAL, METAL_SFX, Volume::linear(1, 4)),
            (VOICE_ELECTRO, BEEP_SFX, Volume::linear(1, 4)),
            (VOICE_FOOTSTEP, FOOTSTEP_SFX, Volume::linear(1, 6)),
            (VOICE_SPLASH, SWOOSH_SFX, Volume::linear(1, 3)),
            (VOICE_ALERT, BEEP_SFX, Volume::linear(1, 2)),
            (VOICE_SPUR, METAL_SFX, Volume::linear(1, 3)),
            (VOICE_CHIME, BEEP_SFX, Volume::linear(1, 3)),
            (VOICE_VOICE, BEEP_SFX, Volume::linear(1, 5)),
        ];

        let mut next_addr = SPU_SAMPLE_BASE;
        for (voice, bytes, vol) in sfx.iter() {
            if let Ok(audio) = Audio::from_bytes(bytes) {
                let addr = SpuAddr::new(next_addr);
                spu::upload_adpcm(addr, audio.adpcm_bytes());
                voice.configure_sample(addr, audio.sample_rate_hz(), *vol, Adsr::sample());
                next_addr += (audio.adpcm_bytes().len() as u32 + 7) & !7;
            }
        }

        // Upload custom title music ADPCM (raw ADPCM blocks, 22050 Hz mono)
        // Convert your MP3 to ADPCM using: ffmpeg -i input.mp3 -ar 22050 -ac 1 -c:a adpcm_psx output.adpcm
        const TITLE_MUSIC_SAMPLE_RATE: u32 = 22050;
        let addr_title = SpuAddr::new(next_addr);
        if !TITLE_MUSIC_ADPCM.is_empty() {
            spu::upload_adpcm(addr_title, TITLE_MUSIC_ADPCM);
            VOICE_TITLE.configure_sample(addr_title, TITLE_MUSIC_SAMPLE_RATE, Volume::linear(3, 4), Adsr::default_tone());
            VOICE_TITLE.set_loop_addr(addr_title);
            next_addr += (TITLE_MUSIC_ADPCM.len() as u32 + 7) & !7;
        }

        unsafe {
            ADDR_TITLE_MUSIC = addr_title;
        }

        // Upload Intro Video audio sample (22050 Hz mono VAG)
        if INTRO_AUDIO_VAG.len() > 48 {
            let addr_intro = SpuAddr::new(next_addr);
            let adpcm_data = &INTRO_AUDIO_VAG[48..];
            spu::upload_adpcm(addr_intro, adpcm_data);
            VOICE_INTRO.configure_sample(addr_intro, 22050, Volume::MAX, Adsr::sample());
            next_addr += (adpcm_data.len() as u32 + 7) & !7;
        }

        // Upload Outro Video audio sample (22050 Hz mono VAG)
        if OUTRO_AUDIO_VAG.len() > 48 {
            let addr_outro = SpuAddr::new(next_addr);
            let adpcm_data = &OUTRO_AUDIO_VAG[48..];
            spu::upload_adpcm(addr_outro, adpcm_data);
            VOICE_OUTRO.configure_sample(addr_outro, 22050, Volume::MAX, Adsr::sample());
            next_addr += (adpcm_data.len() as u32 + 7) & !7;
        }

        // Upload built-in continuous waveform tones for music synthesizer
        let addr_tri = SpuAddr::new(next_addr);
        spu::upload_adpcm(addr_tri, tones::TRIANGLE);
        next_addr += 16;

        let addr_saw = SpuAddr::new(next_addr);
        spu::upload_adpcm(addr_saw, tones::SAWTOOTH);
        next_addr += 16;

        let addr_sqr = SpuAddr::new(next_addr);
        spu::upload_adpcm(addr_sqr, tones::SQUARE);
        next_addr += 16;

        let addr_sin = SpuAddr::new(next_addr);
        spu::upload_adpcm(addr_sin, tones::SINE);

        unsafe {
            ADDR_TRIANGLE = addr_tri;
            ADDR_SAWTOOTH = addr_saw;
            ADDR_SQUARE = addr_sqr;
            ADDR_SINE = addr_sin;
        }

        // Configure Music Voices with looping wave tones
        VOICE_BASS.configure_sample(addr_tri, tones::NATIVE_HZ, Volume::linear(1, 5), Adsr::default_tone());
        VOICE_LEAD.configure_sample(addr_saw, tones::NATIVE_HZ, Volume::linear(1, 6), Adsr::default_tone());
        VOICE_HARMONY.configure_sample(addr_sqr, tones::NATIVE_HZ, Volume::linear(1, 7), Adsr::default_tone());
        VOICE_DRUM.configure_sample(addr_sqr, tones::NATIVE_HZ, Volume::linear(1, 6), Adsr::default_tone());
    }

    pub fn set_bgm(track: BgmTrack) {
        unsafe {
            if AUDIO_STATE.current_track == track {
                return;
            }

            if AUDIO_STATE.current_track == BgmTrack::Title {
                Voice::key_off(VOICE_TITLE.mask());
                TITLE_MUSIC_PLAYING = false;
            }

            AUDIO_STATE.current_track = track;
            AUDIO_STATE.seq_step = 0;
            AUDIO_STATE.tempo_counter = 0;
            AUDIO_STATE.tempo_period = match track {
                BgmTrack::Alert => 5,    // 150 BPM driving pursuit
                BgmTrack::River => 6,    // 130 BPM water runner
                BgmTrack::City => 7,     // 115 BPM urban groove
                BgmTrack::Stealth => 9,  // 90 BPM tense ambient infiltration
                BgmTrack::Beach => 7,    // 115 BPM upbeat surf
                BgmTrack::Boss => 5,     // 150 BPM high stakes
                BgmTrack::Title => 8,    // 100 BPM military overture
                BgmTrack::None => 8,
            };

            if track == BgmTrack::Title {
                VOICE_TITLE.set_start_addr(ADDR_TITLE_MUSIC);
                VOICE_TITLE.set_loop_addr(ADDR_TITLE_MUSIC);
                Voice::key_on(VOICE_TITLE.mask());
                TITLE_MUSIC_PLAYING = true;
            }

            // Mute music voices if track is None
            if track == BgmTrack::None {
                Voice::key_off(VOICE_BASS.mask() | VOICE_LEAD.mask() | VOICE_HARMONY.mask() | VOICE_DRUM.mask());
            }
        }
    }

    /// Frame update called every VBlank to step the hardware music synthesizer.
    pub fn update() {
        unsafe {
            // Handle CODEC incoming call chime animation
            if AUDIO_STATE.chime_timer > 0 {
                AUDIO_STATE.chime_timer -= 1;
                if AUDIO_STATE.chime_timer == 12 {
                    // Second tone of MGS chime (Higher pitch: G5)
                    VOICE_CHIME.set_pitch(Pitch::for_frequency(784, 8000));
                    Voice::key_on(VOICE_CHIME.mask());
                }
            }

            if AUDIO_STATE.current_track == BgmTrack::None {
                return;
            }

            AUDIO_STATE.tempo_counter += 1;
            if AUDIO_STATE.tempo_counter < AUDIO_STATE.tempo_period {
                return;
            }
            AUDIO_STATE.tempo_counter = 0;

            let step = AUDIO_STATE.seq_step as usize;
            AUDIO_STATE.seq_step = (AUDIO_STATE.seq_step + 1) % 16;

            match AUDIO_STATE.current_track {
                BgmTrack::Stealth => {
                    // D Minor Ambient Tactical Bassline: D2, D2, F2, G2, D2, C2, D2, A1
                    let bass_notes = [73, 0, 73, 0, 87, 0, 98, 0, 73, 0, 65, 0, 73, 0, 55, 0];
                    let lead_notes = [0, 293, 0, 349, 0, 293, 0, 440, 0, 392, 0, 349, 0, 293, 0, 261];

                    if bass_notes[step] > 0 {
                        VOICE_BASS.set_pitch(Pitch::for_frequency(bass_notes[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_BASS.mask());
                    }
                    if lead_notes[step] > 0 {
                        VOICE_LEAD.set_pitch(Pitch::for_frequency(lead_notes[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_LEAD.mask());
                    }
                    // Soft heartbeat bass drum on beats 0 and 8
                    if step == 0 || step == 8 {
                        VOICE_DRUM.set_pitch(Pitch::for_frequency(60, tones::NATIVE_HZ));
                        Voice::key_on(VOICE_DRUM.mask());
                    }
                }
                BgmTrack::Alert => {
                    // Fast High-Tension Pursuit (MGS Encounter): Rapid 16th bass & dissonant alarm stabs
                    let bass_alert = [110, 110, 110, 130, 110, 110, 146, 110, 110, 110, 110, 130, 110, 164, 146, 130];
                    let stab_alert = [440, 0, 466, 0, 440, 0, 622, 0, 440, 0, 466, 0, 587, 0, 440, 0];

                    VOICE_BASS.set_pitch(Pitch::for_frequency(bass_alert[step], tones::NATIVE_HZ));
                    Voice::key_on(VOICE_BASS.mask());

                    if stab_alert[step] > 0 {
                        VOICE_LEAD.set_pitch(Pitch::for_frequency(stab_alert[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_LEAD.mask());
                    }

                    // Driving military snare tick on every offbeat
                    if (step % 2) == 1 {
                        VOICE_DRUM.set_pitch(Pitch::for_frequency(2400, tones::NATIVE_HZ));
                        Voice::key_on(VOICE_DRUM.mask());
                    }
                }
                BgmTrack::River => {
                    // Yarra River Runner: Lively flowing melody in A Pentatonic Major
                    let river_bass = [110, 0, 165, 0, 110, 0, 147, 0, 110, 0, 165, 0, 131, 0, 147, 0];
                    let river_lead = [440, 494, 554, 659, 554, 494, 440, 330, 440, 554, 659, 880, 659, 554, 494, 440];

                    if river_bass[step] > 0 {
                        VOICE_BASS.set_pitch(Pitch::for_frequency(river_bass[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_BASS.mask());
                    }
                    VOICE_LEAD.set_pitch(Pitch::for_frequency(river_lead[step], tones::NATIVE_HZ));
                    Voice::key_on(VOICE_LEAD.mask());
                }
                BgmTrack::City => {
                    // Melbourne Highway: 80s Synthwave Bassline in C Minor
                    let city_bass = [65, 65, 131, 65, 78, 78, 156, 78, 87, 87, 175, 87, 98, 98, 196, 98];
                    let city_lead = [523, 0, 466, 0, 392, 0, 349, 0, 523, 0, 587, 0, 659, 0, 523, 0];

                    VOICE_BASS.set_pitch(Pitch::for_frequency(city_bass[step], tones::NATIVE_HZ));
                    Voice::key_on(VOICE_BASS.mask());

                    if city_lead[step] > 0 {
                        VOICE_LEAD.set_pitch(Pitch::for_frequency(city_lead[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_LEAD.mask());
                    }
                }
                BgmTrack::Beach => {
                    // Coastal Dunes: Upbeat Calypso/Surf Groove in G Major
                    let beach_bass = [98, 0, 147, 0, 98, 0, 131, 0, 98, 0, 147, 0, 110, 0, 147, 0];
                    let beach_lead = [392, 440, 494, 587, 494, 440, 392, 0, 494, 587, 784, 587, 494, 392, 440, 392];

                    if beach_bass[step] > 0 {
                        VOICE_BASS.set_pitch(Pitch::for_frequency(beach_bass[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_BASS.mask());
                    }
                    if beach_lead[step] > 0 {
                        VOICE_LEAD.set_pitch(Pitch::for_frequency(beach_lead[step], tones::NATIVE_HZ));
                        Voice::key_on(VOICE_LEAD.mask());
                    }
                }
                BgmTrack::Boss => {
                    // Heavy Mech Boss: Dramatic march with crushing industrial beats
                    let boss_bass = [55, 55, 110, 55, 58, 58, 116, 58, 55, 55, 110, 55, 73, 69, 65, 62];
                    VOICE_BASS.set_pitch(Pitch::for_frequency(boss_bass[step], tones::NATIVE_HZ));
                    Voice::key_on(VOICE_BASS.mask());

                    if step % 4 == 0 {
                        VOICE_DRUM.set_pitch(Pitch::for_frequency(80, tones::NATIVE_HZ));
                        Voice::key_on(VOICE_DRUM.mask());
                    }
                }
                BgmTrack::Title => {
                    // Custom ADPCM title music plays via VOICE_TITLE with hardware looping
                    // No synthesis needed here
                }
                BgmTrack::None => {}
            }
        }
    }

    // -------------------------------------------------------------------------
    // SOUND EFFECTS TRIGGERS
    // -------------------------------------------------------------------------

    #[inline(always)]
    pub fn play_jump() {
        Voice::key_on(VOICE_JUMP.mask());
    }

    #[inline(always)]
    pub fn play_yabby() {
        Voice::key_on(VOICE_YABBY.mask());
    }

    #[inline(always)]
    pub fn play_swoosh() {
        Voice::key_on(VOICE_SWOOSH.mask());
    }

    #[inline(always)]
    pub fn play_hit() {
        Voice::key_on(VOICE_HIT.mask());
    }

    #[inline(always)]
    pub fn play_metal() {
        Voice::key_on(VOICE_METAL.mask());
    }

    #[inline(always)]
    pub fn play_electro() {
        Voice::key_on(VOICE_ELECTRO.mask());
    }

    #[inline]
    pub fn play_footstep(surface: SurfaceType) {
        let (pitch_hz, vol) = match surface {
            SurfaceType::Metal => (14000, Volume::linear(1, 3)),
            SurfaceType::Concrete => (10000, Volume::linear(1, 5)),
            SurfaceType::Grass => (6500, Volume::linear(1, 8)),
            SurfaceType::Water => (4500, Volume::linear(1, 6)),
        };
        VOICE_FOOTSTEP.set_volume(vol, vol);
        VOICE_FOOTSTEP.set_pitch(Pitch::for_frequency(pitch_hz, 11025));
        Voice::key_on(VOICE_FOOTSTEP.mask());
    }

    #[inline]
    pub fn play_splash() {
        VOICE_SPLASH.set_pitch(Pitch::for_frequency(6000, 11025));
        Voice::key_on(VOICE_SPLASH.mask());
    }

    /// Iconic MGS Sentry alert exclamation chord (`!`).
    #[inline]
    pub fn play_alert() {
        // High sharp dissonant sting
        VOICE_ALERT.set_pitch(Pitch::for_frequency(18000, 8000));
        Voice::key_on(VOICE_ALERT.mask());
    }

    /// Venomous spur strike CQC takedown.
    #[inline]
    pub fn play_spur() {
        VOICE_SPUR.set_pitch(Pitch::for_frequency(14000, 11025));
        Voice::key_on(VOICE_SPUR.mask());
    }

    /// MGS CODEC incoming call chime (BEEP-BEEP).
    pub fn play_codec_chime() {
        unsafe {
            AUDIO_STATE.chime_timer = 24;
        }
        // First tone: E5 (659 Hz)
        VOICE_CHIME.set_pitch(Pitch::for_frequency(659, 8000));
        Voice::key_on(VOICE_CHIME.mask());
    }

    /// Radio voice chatter chirp during dialogue scrolling.
    #[inline]
    pub fn play_codec_chirp(char_idx: usize) {
        let pitch = 8000 + ((char_idx as u32 * 37) % 3000);
        VOICE_VOICE.set_pitch(Pitch::for_frequency(pitch, 8000));
        Voice::key_on(VOICE_VOICE.mask());
    }

    #[inline(always)]
    pub fn play_fanfare() {
        Voice::key_on(VOICE_SELECT.mask());
    }

    /// Play CD-DA track 2 (title music).
    pub fn play_cdda_title() {
        cdrom::play_track(2);
    }

    /// Stop CD-DA playback.
    pub fn stop_cdda() {
        cdrom::stop();
    }

    /// Pause CD-DA playback.
    pub fn pause_cdda() {
        cdrom::pause();
    }

    /// Check if CD-DA is currently playing.
    pub fn is_cdda_playing() -> bool {
        if let Some(resp) = cdrom::try_get_stat(1000) {
            let status = resp.bytes().first().copied().unwrap_or(0);
            let playing = status & cdrom::STAT_PLAYING != 0;
            unsafe {
                if playing {
                    CDDA_WAS_PLAYING = true;
                }
            }
            playing
        } else {
            false
        }
    }

    /// Check if CD-DA was playing but has now finished (for attract demo trigger).
    pub fn cdda_finished() -> bool {
        unsafe {
            CDDA_WAS_PLAYING && !Self::is_cdda_playing()
        }
    }

    /// Reset CDDA play tracking (call when starting title music).
    pub fn reset_cdda_tracking() {
        unsafe {
            CDDA_WAS_PLAYING = false;
        }
    }

    /// Play cinematic intro video audio (SPU ADPCM sample).
    pub fn play_intro_audio() {
        Voice::key_on(VOICE_INTRO.mask());
    }

    /// Stop cinematic intro video audio.
    pub fn stop_intro_audio() {
        Voice::key_off(VOICE_INTRO.mask());
    }

    /// Play cinematic outro video audio (SPU ADPCM sample).
    pub fn play_outro_audio() {
        Voice::key_on(VOICE_OUTRO.mask());
    }

    /// Stop cinematic outro video audio.
    pub fn stop_outro_audio() {
        Voice::key_off(VOICE_OUTRO.mask());
    }
}
