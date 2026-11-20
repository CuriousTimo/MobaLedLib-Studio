//! LED simulator: the engine of MobaLedLib 3.5.0 compiled for the computer (see `build.rs`).
//!
//! It takes the bytes of a `Config[]` table, as the Excel tool's `MobaLedLibWrapper.dll` does, and runs
//! `MobaLedLib_C::Update()` at the times the caller sets. The engine keeps global state, so there is one
//! simulator at a time.
//!
//! Not compared with the Excel tool's simulator yet.

use std::sync::atomic::{AtomicBool, Ordering};

mod ffi {
    extern "C" {
        pub fn mll_sim_create(config: *const u8, config_len: u16, ram_size: u16, num_leds: u16);
        pub fn mll_sim_destroy();
        pub fn mll_sim_set_millis(t_ms: u32);
        pub fn mll_sim_update();
        pub fn mll_sim_set_input(channel: u8, on: u8);
        pub fn mll_sim_get_input(channel: u8) -> i8;
        pub fn mll_sim_led(i: u16, rgb: *mut u8);
    }
}

static IN_USE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Another simulator exists; the engine has global state.
    Busy,
    /// The configuration is longer than 65535 bytes.
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Busy => write!(f, "another simulator is running"),
            Error::TooLarge => write!(f, "the configuration is larger than 65535 bytes"),
        }
    }
}

impl std::error::Error for Error {}

/// One running MobaLedLib engine.
pub struct Simulator {
    num_leds: u16,
}

impl Simulator {
    /// Starts the engine with the bytes of `Config[]`, `ram_size` bytes of RAM for it (`Config_RAM`, the
    /// `__COUNTER__ / 2` of `MobaLedLib_Create`) and `num_leds` LEDs. The time starts at 0 ms.
    pub fn new(config: &[u8], ram_size: u16, num_leds: u16) -> Result<Simulator, Error> {
        let len = u16::try_from(config.len()).map_err(|_| Error::TooLarge)?;
        if IN_USE.swap(true, Ordering::SeqCst) {
            return Err(Error::Busy);
        }
        // SAFETY: the C side copies the config; only one simulator exists (IN_USE).
        unsafe { ffi::mll_sim_create(config.as_ptr(), len, ram_size, num_leds) };
        Ok(Simulator { num_leds })
    }

    /// Sets the time `millis()` returns, in ms since the start.
    pub fn set_time(&mut self, ms: u32) {
        unsafe { ffi::mll_sim_set_millis(ms) }
    }

    /// One `MobaLedLib_C::Update()`.
    pub fn update(&mut self) {
        unsafe { ffi::mll_sim_update() }
    }

    /// `MobaLedLib_C::Set_Input(channel, on)`.
    pub fn set_input(&mut self, channel: u8, on: bool) {
        unsafe { ffi::mll_sim_set_input(channel, on as u8) }
    }

    /// `MobaLedLib_C::Get_Input(channel)`: the input state (`INP_OFF`, `INP_ON`, `INP_TURNED_ON`, …).
    pub fn input(&self, channel: u8) -> i8 {
        unsafe { ffi::mll_sim_get_input(channel) }
    }

    pub fn num_leds(&self) -> u16 {
        self.num_leds
    }

    /// The colour of LED `i` as `[r, g, b]`; black for a number past the end.
    pub fn led(&self, i: u16) -> [u8; 3] {
        let mut rgb = [0u8; 3];
        unsafe { ffi::mll_sim_led(i, rgb.as_mut_ptr()) };
        rgb
    }

    /// The colours of all LEDs.
    pub fn leds(&self) -> Vec<[u8; 3]> {
        (0..self.num_leds).map(|i| self.led(i)).collect()
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        unsafe { ffi::mll_sim_destroy() };
        IN_USE.store(false, Ordering::SeqCst);
    }
}
