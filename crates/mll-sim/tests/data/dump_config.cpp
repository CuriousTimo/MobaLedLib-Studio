// Makes adorf_am328.bin from LEDs_AutoProg.h (written by `mll generate "examples/Adorf station.json"`):
// writes Config[] to config.bin and prints the RAM size and the LED count used in tests/engine.rs.
//
//   V=crates/mll-sim/vendor
//   clang++ -std=gnu++17 -w -DFASTLED_STUB_IMPL -DFASTLED_NO_PINMAP -Icrates/mll-sim/csrc/shim \
//     -Icrates/mll-sim/tests/data -I$V/MobaLedLib/src -I$V/FastLED/src crates/mll-sim/tests/data/dump_config.cpp \
//     -o dump_config && ./dump_config
#include "LEDs_AutoProg.h"
static const unsigned RAM_SIZE = __COUNTER__ / 2;
SimSerial Serial;
int main() {
  FILE *f = fopen("config.bin", "wb");
  fwrite(Config, 1, sizeof(Config), f);
  fclose(f);
  printf("ram_size=%u num_leds=%d config_len=%zu\n", RAM_SIZE, NUM_LEDS, sizeof(Config));
}
