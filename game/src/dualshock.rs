//! DualShock analog & vibration controller driver for Plattypus MGS.
//! Direct SIO0 protocol control with actuator mapping (Command 0x4D)
//! and dual-motor rumble polling.

use psx_hw::sio::sio0;
use psx_io::sio;
use psx_pad::{AnalogSticks, ButtonState, Deadzone, PadMode, PadState};

const DEFAULT_SETUP_SPINS: u32 = 1_024;
const EXCHANGE_WAIT_SPINS: u32 = 32_768;
const CONFIG_COMMAND_GAP_SPINS: u32 = 8 * DEFAULT_SETUP_SPINS;

const MODE_8N1: u16 = sio0::MODE_8N1;
const BAUD_PAD: u16 = sio0::BAUD_250KHZ;
const CTRL_ACK: u16 = sio0::ctrl::ACK;

const STAT_TX_READY: u32 = sio0::stat::TX_READY;
const STAT_RX_NOT_EMPTY: u32 = sio0::stat::RX_NOT_EMPTY;

pub struct DualShockController {
    pub is_analog: bool,
    pub deadzone: Deadzone,
}

impl DualShockController {
    pub const fn new() -> Self {
        Self {
            is_analog: false,
            deadzone: Deadzone::new(18),
        }
    }

    /// Initialize DualShock analog mode and configure vibration actuators.
    pub fn init(&mut self) -> bool {
        self.is_analog = init_dualshock_actuators();
        self.is_analog
    }

    /// Poll Port 1 with vibration commands for both motors.
    /// - `small_motor`: High-frequency small motor (true = spin, false = off).
    /// - `large_motor`: Low-frequency weighted motor (0 = off, 1..255 = speed/intensity).
    pub fn poll(&self, small_motor: bool, large_motor: u8) -> PadState {
        poll_port1_rumble(small_motor, large_motor)
    }
}

/// Configure DualShock: enter config mode, set analog locked mode, map vibration actuators (0x4D), exit config mode.
fn init_dualshock_actuators() -> bool {
    unsafe {
        // Enter config mode (0x43)
        transaction(false, [0x43, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Lock analog mode (0x44)
        transaction(false, [0x44, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Configure actuator mapping (0x4D):
        // Byte 2 = 0x00 (map small motor to byte index 0 of poll payload)
        // Byte 3 = 0x01 (map large motor to byte index 1 of poll payload)
        // Bytes 4..7 = 0xFF (unmapped)
        transaction(false, [0x4D, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Exit config mode (0x43)
        transaction(false, [0x43, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);
    }

    // Verify current mode
    let s = poll_port1_rumble(false, 0);
    s.is_analog()
}

/// Poll controller in Port 1, retrying garbled reads.
pub fn poll_port1_rumble(small_motor: bool, large_motor: u8) -> PadState {
    let mut last = PadState::NONE;
    let mut tries = 0;
    let motor0 = if small_motor { 0x01 } else { 0x00 };
    let motor1 = large_motor;

    while tries < 4 {
        let s = unsafe { poll_port1_rumble_raw(motor0, motor1) };
        if !s.is_connected() {
            return s; // No controller connected
        }
        if s.mode != PadMode::Unknown {
            return s; // Clean read
        }
        last = s;
        tries += 1;
    }
    last
}

/// Execute a single poll transaction with vibration motor outputs.
unsafe fn poll_port1_rumble_raw(motor0: u8, motor1: u8) -> PadState {
    unsafe {
        select(false);
        delay_reads(DEFAULT_SETUP_SPINS);
        drain_rx();

        let _select = exchange(0x01);
        let id_low = exchange(0x42);
        let mut mode = mode_from_id_low(id_low);
        if !mode.is_connected() {
            deselect();
            return PadState {
                buttons: ButtonState::NONE,
                mode: PadMode::Disconnected,
                sticks: AnalogSticks::CENTERED,
                id_low,
            };
        }

        let analog = mode.has_sticks();
        let id_high = exchange(0x00);
        if id_high != 0x5A {
            mode = PadMode::Unknown;
        }
        let read_sticks = analog && mode != PadMode::Unknown;

        // While reading button byte 0, transmit motor0 value (small motor)
        let b0 = exchange(motor0);
        // While reading button byte 1, transmit motor1 value (large motor)
        let b1 = exchange(motor1);

        let sticks = if read_sticks {
            let right_x = exchange(0x00);
            let right_y = exchange(0x00);
            let left_x = exchange(0x00);
            let left_y = exchange(0x00);
            AnalogSticks {
                right_x,
                right_y,
                left_x,
                left_y,
            }
        } else {
            AnalogSticks::CENTERED
        };

        deselect();

        let buttons = ButtonState::from_bits(!((b0 as u16) | ((b1 as u16) << 8)));

        PadState {
            buttons,
            mode,
            sticks,
            id_low,
        }
    }
}

#[inline]
unsafe fn select(port2: bool) {
    unsafe {
        psx_io::write16(sio::MODE, MODE_8N1);
        psx_io::write16(sio::BAUD, BAUD_PAD);
        psx_io::write16(sio::CTRL, CTRL_ACK);
        psx_io::write16(sio::CTRL, sio0::selected_ctrl(port2, false));
    }
}

#[inline]
unsafe fn deselect() {
    unsafe { psx_io::write16(sio::CTRL, 0) };
}

#[inline]
unsafe fn drain_rx() {
    let mut n = 0;
    unsafe {
        while psx_io::read32(sio::STAT) & STAT_RX_NOT_EMPTY != 0 && n < 16 {
            let _ = psx_io::read8(sio::DATA);
            n += 1;
        }
    }
}

#[inline]
unsafe fn exchange(tx: u8) -> u8 {
    unsafe {
        if !wait_stat(STAT_TX_READY, EXCHANGE_WAIT_SPINS) {
            return 0xFF;
        }
        psx_io::write8(sio::DATA, tx);
        if !wait_stat(STAT_RX_NOT_EMPTY, EXCHANGE_WAIT_SPINS) {
            return 0xFF;
        }
        psx_io::read8(sio::DATA)
    }
}

#[inline]
unsafe fn wait_stat(mask: u32, spins: u32) -> bool {
    let mut spins = spins;
    unsafe {
        while psx_io::read32(sio::STAT) & mask == 0 {
            if spins == 0 {
                return false;
            }
            spins -= 1;
            core::hint::spin_loop();
        }
    }
    true
}

#[inline]
unsafe fn delay_reads(n: u32) {
    let mut k = n;
    unsafe {
        while k > 0 {
            let _ = psx_io::read32(sio::STAT);
            k -= 1;
            core::hint::spin_loop();
        }
    }
}

#[inline]
unsafe fn transaction(port2: bool, bytes: [u8; 8]) -> [u8; 8] {
    unsafe {
        select(port2);
        delay_reads(DEFAULT_SETUP_SPINS);
        let _select = exchange(0x01);
        let mut out = [0u8; 8];
        let mut i = 0;
        while i < bytes.len() {
            out[i] = exchange(bytes[i]);
            i += 1;
        }
        deselect();
        out
    }
}

#[inline]
fn mode_from_id_low(id_low: u8) -> PadMode {
    match id_low {
        0x41 => PadMode::Digital,
        0x73 => PadMode::Analog,
        0xF3 => PadMode::Config,
        0xFF => PadMode::Disconnected,
        _ => PadMode::Unknown,
    }
}
