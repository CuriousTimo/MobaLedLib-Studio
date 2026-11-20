// Stand-in for the Arduino core, so that the MobaLedLib engine compiles on the computer for the simulator.
// Only what the engine uses. The time (millis) is set by the simulator; random() is avr-libc's (sim.cpp).
#pragma once
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <math.h>

typedef uint8_t byte;
typedef bool boolean;

#define PROGMEM
#define pgm_read_byte(a)       (*(const uint8_t  *)(a))
#define pgm_read_byte_near(a)  (*(const uint8_t  *)(a))
#define pgm_read_word_near(a)  (*(const uint16_t *)(a))
#define pgm_read_dword_near(a) (*(const uint32_t *)(a))
#define F(s) (s)
class __FlashStringHelper;

// Declared as FastLED's stub platform declares them (platforms/stub/led_sysdefs_stub_generic.h)
extern "C" {
  uint32_t millis(void);
  uint32_t micros(void);
  void delay(int ms);
  void pinMode(uint8_t pin, uint8_t mode);
}

long random(long howbig);
long random(long howsmall, long howbig);
void randomSeed(unsigned long seed);
int analogRead(uint8_t pin);

#define HIGH 1
#define LOW 0
#define INPUT 0
#define OUTPUT 1
#define INPUT_PULLUP 2
#define LED_BUILTIN 13
inline int digitalRead(uint8_t) { return 0; }
inline void digitalWrite(uint8_t, uint8_t) {}

// The engine's debug output goes nowhere
struct SimSerial {
  template <class T> void print(T) {}
  template <class T> void println(T) {}
  void println() {}
  void begin(long) {}
  void flush() {}
};
extern SimSerial Serial;

#ifndef min
#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))
#endif
