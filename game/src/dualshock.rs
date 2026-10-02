//! DualShock analog & vibration controller driver for Plattypus MGS.
//! Direct SIO0 protocol control with actuator mapping (Command 0x4D)
//! and dual-motor rumble polling.

use psx_hw::sio::sio0;
use psx_io::sio;
use psx_pad::{AnalogSticks, ButtonState, Deadzone, PadMode, PadState};

const DEFAULT_SETUP_SPINS: u32 = 1_024;
const CONFIG_COMMAND_GAP_SPINS: u32 = 8 * DEFAULT_SETUP_SPINS;

/// Spin budget for the address + ID bytes, the only exchanges needed to decide
/// whether a pad is present. An empty port never asserts RX_NOT_EMPTY, so this
/// is the budget that bounds the per-frame cost of a disconnected controller.
/// 1_024 is the SDK's on-console-calibrated `DEFAULT_SETUP_SPINS`: its sweep put
/// a real SCPH-1200's first response between 384 (no response) and 768 (clean),
/// so this clears the measured hardware floor with margin.
const PROBE_WAIT_SPINS: u32 = 1_024;

/// Spin budget for the remaining bytes of a transfer, used only once the ID
/// handshake has already confirmed a pad is answering. 2_048 matches the SDK's
/// `ACK_WAIT_SPINS`, which it documents as comfortably exceeding the kernel's
/// ~100us DSR timeout on hardware, so a slow pad still gets time to answer.
const EXCHANGE_WAIT_SPINS: u32 = 2_048;

/// Value written to the small motor's byte in a poll transfer when it should be
/// spinning. 0xFF rather than 0x01: the channel is documented both as "bit0 set
/// = on" and as "the byte must be 0xFF", and 0xFF satisfies both readings while
/// 0x01 satisfies only the first. The large motor takes its intensity directly.
const MOTOR_SMALL_ON: u8 = 0xFF;

const MODE_8N1: u16 = sio0::MODE_8N1;
const BAUD_PAD: u16 = sio0::BAUD_250KHZ;
const CTRL_ACK: u16 = sio0::ctrl::ACK;

const STAT_TX_READY: u32 = sio0::stat::TX_READY;
const STAT_RX_NOT_EMPTY: u32 = sio0::stat::RX_NOT_EMPTY;

pub struct DualShockController {
    pub is_analog: bool,
    pub deadzone: Deadzone,
    pub active_port: bool,
}

impl DualShockController {
    pub const fn new() -> Self {
        Self {
            is_analog: false,
            deadzone: Deadzone::new(18),
            active_port: false,
        }
    }

    /// Initialize DualShock analog mode and configure vibration actuators.
    /// Probes Port 1 first, falling back to Port 2 if Port 1 is disconnected.
    pub fn init(&mut self) -> bool {
        if init_dualshock_actuators(false) {
            self.active_port = false;
            self.is_analog = true;
            return true;
        }
        let s1 = poll_port_rumble(false, false, 0);
        if s1.is_connected() {
            self.active_port = false;
            self.is_analog = s1.is_analog();
            return self.is_analog;
        }

        if init_dualshock_actuators(true) {
            self.active_port = true;
            self.is_analog = true;
            return true;
        }
        let s2 = poll_port_rumble(true, false, 0);
        if s2.is_connected() {
            self.active_port = true;
            self.is_analog = s2.is_analog();
            return self.is_analog;
        }

        self.active_port = false;
        self.is_analog = false;
        false
    }

    /// Poll active controller port with vibration commands for both motors.
    /// Supports automatic failover / hot-swap to the other port if disconnected.
    /// - `small_motor`: High-frequency small motor (true = spin, false = off).
    /// - `large_motor`: Low-frequency weighted motor (0 = off, 1..255 = speed/intensity).
    pub fn poll(&mut self, small_motor: bool, large_motor: u8) -> PadState {
        let s = poll_port_rumble(self.active_port, small_motor, large_motor);
        if s.is_connected() {
            return s;
        }

        // SE-4: Port failover / hot-swap to alternate port
        let alt_port = !self.active_port;
        let alt_s = poll_port_rumble(alt_port, small_motor, large_motor);
        if alt_s.is_connected() {
            // SE-12: The pad latches the last motor bytes it was sent and keeps
            // spinning at that intensity until something overwrites them, and
            // 0x4D only maps channels, it never stops them -- so the port we
            // abandon here would rumble for the rest of the session. Zero it
            // before switching. Load-bearing only because the motor bytes sent
            // during polling are actually honoured (see `init_dualshock_actuators`).
            stop_motors(self.active_port);
            self.active_port = alt_port;
            self.is_analog = init_dualshock_actuators(alt_port);
            return alt_s;
        }

        // SE-12: Nothing in either port. Both were just written this frame, but
        // with the *requested* motor bytes, so a pad that merely glitched a
        // handshake this frame would still be left spinning. Re-zero both.
        stop_motors(self.active_port);
        stop_motors(alt_port);
        s
    }
}

/// Command both motors off on `port2` with a single poll transaction.
/// The result is discarded: every caller is either abandoning the port or has
/// already lost it, and a zeroed poll cannot fail in a way that matters.
fn stop_motors(port2: bool) {
    unsafe { poll_port_rumble_raw(port2, 0, 0) };
}

/// Configure DualShock: enter config mode, set analog locked mode, map vibration actuators (0x4D), exit config mode.
fn init_dualshock_actuators(port2: bool) -> bool {
    unsafe {
        // Enter config mode (0x43)
        transaction(port2, [0x43, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Lock analog mode (0x44)
        transaction(port2, [0x44, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Set actuator map (0x4D, "PadSetActAlign"). Byte 1 is the config-mode
        // 0x00 filler; bytes 2..7 are one *mapping selector per data byte of the
        // 0x42 poll*, not a count and not a flag word:
        //   0x00 = map the small motor (M2) to bit0 of that poll byte
        //   0x01 = map the large motor (M1) to bits 0..7 of that poll byte
        //   0xFF = map that poll byte to nothing
        // So 0x00/0x01/FF... is the documented two-motor enable, and it makes the
        // poll's motor bytes live: byte 2 drives motor0, byte 3 drives motor1.
        transaction(port2, [0x4D, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);

        // Exit config mode (0x43)
        transaction(port2, [0x43, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        delay_reads(CONFIG_COMMAND_GAP_SPINS);
    }

    // Verify current mode
    let s = poll_port_rumble(port2, false, 0);
    s.is_analog()
}

/// Poll controller in Port 1, retrying garbled reads.
pub fn poll_port1_rumble(small_motor: bool, large_motor: u8) -> PadState {
    poll_port_rumble(false, small_motor, large_motor)
}

/// Poll controller in specified port (false = Port 1, true = Port 2), retrying garbled reads.
pub fn poll_port_rumble(port2: bool, small_motor: bool, large_motor: u8) -> PadState {
    let mut last = PadState::NONE;
    let mut tries = 0;
    let motor0 = if small_motor { 0x01 } else { 0x00 };
    let motor1 = large_motor;

    while tries < 4 {
        let s = unsafe { poll_port_rumble_raw(port2, motor0, motor1) };
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
unsafe fn poll_port_rumble_raw(port2: bool, motor0: u8, motor1: u8) -> PadState {
    unsafe {
        select(port2);
        delay_reads(DEFAULT_SETUP_SPINS);
        drain_rx();

        let _select = exchange(0x01, PROBE_WAIT_SPINS);
        let id_low = exchange(0x42, PROBE_WAIT_SPINS);
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
        let id_high = exchange(0x00, EXCHANGE_WAIT_SPINS);
        if id_high != 0x5A {
            mode = PadMode::Unknown;
        }
        let read_sticks = analog && mode != PadMode::Unknown;

        // While reading button byte 0, transmit motor0 value (small motor)
        let b0 = exchange(
            if motor0 != 0 { MOTOR_SMALL_ON } else { 0x00 },
            EXCHANGE_WAIT_SPINS,
        );
        // While reading button byte 1, transmit motor1 value (large motor)
        let b1 = exchange(motor1, EXCHANGE_WAIT_SPINS);

        let sticks = if read_sticks {
            let right_x = exchange(0x00, EXCHANGE_WAIT_SPINS);
            let right_y = exchange(0x00, EXCHANGE_WAIT_SPINS);
            let left_x = exchange(0x00, EXCHANGE_WAIT_SPINS);
            let left_y = exchange(0x00, EXCHANGE_WAIT_SPINS);
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
unsafe fn exchange(tx: u8, spins: u32) -> u8 {
    unsafe {
        if !wait_stat(STAT_TX_READY, spins) {
            return 0xFF;
        }
        psx_io::write8(sio::DATA, tx);
        if !wait_stat(STAT_RX_NOT_EMPTY, spins) {
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
        let _select = exchange(0x01, EXCHANGE_WAIT_SPINS);
        let mut out = [0u8; 8];
        let mut i = 0;
        while i < bytes.len() {
            out[i] = exchange(bytes[i], EXCHANGE_WAIT_SPINS);
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
