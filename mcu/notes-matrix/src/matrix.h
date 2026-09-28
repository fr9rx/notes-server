/* 8x13 charlieplexed LED matrix of the Arduino UNO Q. */
#pragma once

#include <stdint.h>

#define MATRIX_W    13
#define MATRIX_H    8
#define MATRIX_LEDS (MATRIX_W * MATRIX_H)
#define MATRIX_MAX  7 /* brightness levels 0..7 */

/* Configures the GPIOs and starts the scan timer. Returns 0 or -errno. */
int matrix_init(void);

/* Shows a frame of MATRIX_LEDS brightness values (row-major, 0..7). */
void matrix_show(const uint8_t frame[MATRIX_LEDS]);
