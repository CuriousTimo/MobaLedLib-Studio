//! The engine runs the Config[] of the example layout `examples/Adorf station.json` (tests/data, made with
//! tests/data/dump_config.cpp). These tests check that the engine runs; they are not compared with the Excel
//! tool's simulator.

use std::sync::Mutex;

use mll_sim::{Error, Simulator};

const CONFIG: &[u8] = include_bytes!("data/adorf_am328.bin");
const RAM_SIZE: u16 = 81;
const NUM_LEDS: u16 = 25;

const HEARTBEAT: u16 = 0; // RGB_Heartbeat(0)
                          // GasLights(6, 1, GAS_LIGHT ×4): LEDs 6..9 on input channel 1; a House, so the lamps go on one by one at
                          // random times
const GAS_LAMPS: std::ops::Range<u16> = 6..10;
const GAS_INPUT: u8 = 1;

// The engine has global state: one test at a time.
static ENGINE: Mutex<()> = Mutex::new(());

/// Runs the engine every 10 ms up to `until` ms and returns the LEDs at every 500 ms.
fn run(sim: &mut Simulator, until: u32) -> Vec<Vec<[u8; 3]>> {
    let mut frames = Vec::new();
    for t in (0..=until).step_by(10) {
        sim.set_time(t);
        sim.update();
        if t % 500 == 0 {
            frames.push(sim.leds());
        }
    }
    frames
}

#[test]
fn heartbeat_changes() {
    let _g = ENGINE.lock().unwrap();
    let mut sim = Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).unwrap();
    let frames = run(&mut sim, 3000);
    let beat: Vec<[u8; 3]> = frames.iter().map(|f| f[HEARTBEAT as usize]).collect();
    assert!(beat.windows(2).any(|w| w[0] != w[1]), "{beat:?}");
}

#[test]
fn gas_lamp_follows_its_input() {
    let _g = ENGINE.lock().unwrap();
    let mut sim = Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).unwrap();
    run(&mut sim, 3000);
    assert!(GAS_LAMPS.clone().all(|i| sim.led(i) == [0, 0, 0]));
    drop(sim);

    let mut sim = Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).unwrap();
    sim.set_input(GAS_INPUT, true);
    run(&mut sim, 3000);
    assert!(GAS_LAMPS.clone().any(|i| sim.led(i) != [0, 0, 0]));
}

#[test]
fn same_inputs_same_leds() {
    let _g = ENGINE.lock().unwrap();
    let trace = || {
        let mut sim = Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).unwrap();
        for ch in 0..12 {
            sim.set_input(ch, true);
        }
        run(&mut sim, 5000)
    };
    assert_eq!(trace(), trace());
}

#[test]
fn one_simulator_at_a_time() {
    let _g = ENGINE.lock().unwrap();
    let sim = Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).unwrap();
    assert_eq!(Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).err(), Some(Error::Busy));
    drop(sim);
    assert!(Simulator::new(CONFIG, RAM_SIZE, NUM_LEDS).is_ok());
}
