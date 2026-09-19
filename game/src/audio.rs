//! Audio engine using the PS1 SPU and baked PSAU samples.

use psx_asset::Audio;
use psx_spu::{self as spu, Adsr, SpuAddr, Voice, Volume};

static JUMP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/jump.psau");
static COIN_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/pickup_coin.psau");
static SWOOSH_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/swoosh.psau");
static PUNCH_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/hit_punch.psau");
static METAL_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/hit_metal.psau");
static BEEP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/ui_beep.psau");
static FOOTSTEP_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/footstep.psau");
static SELECT_SFX: &[u8] = include_bytes!("../../psoxide/assets/audio/freesfx/psau/ui_select.psau");

const SPU_SAMPLE_BASE: u32 = 0x1010;

pub const VOICE_JUMP: Voice = Voice::V0;
pub const VOICE_YABBY: Voice = Voice::V1;
pub const VOICE_SWOOSH: Voice = Voice::V2;
pub const VOICE_HIT: Voice = Voice::V3;
pub const VOICE_METAL: Voice = Voice::V4;
pub const VOICE_ELECTRO: Voice = Voice::V5;
pub const VOICE_WADDLE: Voice = Voice::V6;
pub const VOICE_SELECT: Voice = Voice::V7;

pub struct AudioManager;

impl AudioManager {
    pub fn init() {
        spu::init();
        spu::set_main_volume(Volume::MAX, Volume::MAX);

        let sfx = [
            (VOICE_JUMP, JUMP_SFX, Volume::linear(1, 4)),
            (VOICE_YABBY, COIN_SFX, Volume::linear(1, 4)),
            (VOICE_SWOOSH, SWOOSH_SFX, Volume::linear(1, 3)),
            (VOICE_HIT, PUNCH_SFX, Volume::linear(1, 3)),
            (VOICE_METAL, METAL_SFX, Volume::linear(1, 4)),
            (VOICE_ELECTRO, BEEP_SFX, Volume::linear(1, 4)),
            (VOICE_WADDLE, FOOTSTEP_SFX, Volume::linear(1, 6)),
            (VOICE_SELECT, SELECT_SFX, Volume::linear(1, 3)),
        ];

        let mut next_addr = SPU_SAMPLE_BASE;
        for (voice, bytes, vol) in sfx.iter() {
            if let Ok(audio) = Audio::from_bytes(bytes) {
                let addr = SpuAddr::new(next_addr);
                spu::upload_adpcm(addr, audio.adpcm_bytes());
                voice.configure_sample(addr, audio.sample_rate_hz(), *vol, Adsr::sample());
                next_addr += audio.adpcm_bytes().len() as u32;
            }
        }
    }

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

    #[inline(always)]
    pub fn play_waddle() {
        Voice::key_on(VOICE_WADDLE.mask());
    }

    #[inline(always)]
    pub fn play_fanfare() {
        Voice::key_on(VOICE_SELECT.mask());
    }
}
