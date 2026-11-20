// The simulator: the engine of MobaLedLib 3.5.0 (vendor/MobaLedLib/src) on the computer, with the time set
// by the caller. One simulator at a time, like the Excel tool's MobaLedLibWrapper.dll (Create, Update,
// SetInput, GetInput, GetLedColor).
//
// The engine sources are compiled as one unit: MobaLedLib.cpp defines Get_Input() inline and the other
// files call it.
#include "MobaLedLib.cpp"
#include "House.cpp"
#include "AnalogPattern.cpp"
#include "Counter.cpp"
#include "Logic.cpp"
#include "RandFunc.cpp"
#include "Schedule.cpp"
#include "WeldingLight.cpp"

SimSerial Serial;

static uint32_t sim_ms = 0;
extern "C" uint32_t millis(void) { return sim_ms; }
extern "C" uint32_t micros(void) { return sim_ms * 1000; }
extern "C" void delay(int) {}
extern "C" void pinMode(uint8_t, uint8_t) {}
int analogRead(uint8_t) { return 0; }

// random() of avr-libc (libc/stdlib/random.c), as on the Nano, and Arduino's random(n) and random(a, b)
// and randomSeed() (cores/arduino/WMath.cpp)
static unsigned long rnd_next = 1;
static long avr_random() {
  long hi, lo, x = (long)rnd_next;
  if (x == 0) x = 123459876L;
  hi = x / 127773L;
  lo = x % 127773L;
  x = 16807L * lo - 2836L * hi;
  if (x < 0) x += 0x7fffffffL;
  rnd_next = (unsigned long)x;
  return (long)((unsigned long)x % (0x7fffffffUL + 1));
}
long random(long howbig) { return howbig == 0 ? 0 : avr_random() % howbig; }
long random(long howsmall, long howbig) {
  return howsmall >= howbig ? howsmall : random(howbig - howsmall) + howsmall;
}
void randomSeed(unsigned long seed) {
  if (seed != 0) rnd_next = seed;
}

static MobaLedLib_C *mll = nullptr;
static CRGB *leds = nullptr;
static uint8_t *cfg = nullptr, *ram = nullptr;
static uint16_t led_cnt = 0;

extern "C" void mll_sim_destroy(void) {
  delete mll;
  delete[] leds;
  delete[] cfg;
  delete[] ram;
  mll = nullptr;
  leds = nullptr;
  cfg = ram = nullptr;
  led_cnt = 0;
}

extern "C" void mll_sim_create(const uint8_t *config, uint16_t config_len, uint16_t ram_size, uint16_t num_leds) {
  mll_sim_destroy();
  // The global state as after switching on: the start values of their definitions
  sim_ms = 0;
  rnd_next = 1;                 // avr-libc random.c
  rand16seed = 1337;            // FastLED lib8tion.cpp: RAND16_SEED
  Darkness = 0;                 // MobaLedLib.cpp
  DayState = Unknown;
  TestMode = 0;
  cfg = new uint8_t[config_len ? config_len : 1];
  memcpy(cfg, config, config_len);
  ram = new uint8_t[ram_size ? ram_size : 1]();
  leds = new CRGB[num_leds ? num_leds : 1]();
  led_cnt = num_leds;
  mll = new MobaLedLib_C(leds, num_leds, cfg, ram, ram_size, nullptr, nullptr);
}

extern "C" void mll_sim_set_millis(uint32_t t_ms) { sim_ms = t_ms; }
extern "C" void mll_sim_update(void) {
  if (mll) mll->Update();
}
extern "C" void mll_sim_set_input(uint8_t channel, uint8_t on) {
  if (mll) mll->Set_Input(channel, on);
}
extern "C" int8_t mll_sim_get_input(uint8_t channel) { return mll ? mll->Get_Input(channel) : 0; }
extern "C" void mll_sim_led(uint16_t i, uint8_t rgb[3]) {
  if (mll && i < led_cnt) {
    rgb[0] = leds[i].r;
    rgb[1] = leds[i].g;
    rgb[2] = leds[i].b;
  } else {
    rgb[0] = rgb[1] = rgb[2] = 0;
  }
}
