/*
 * What the matrix shows, driven by notes-server's status updates (delivered
 * by rpc.c). Plain C (no Zephyr APIs) so it can be compiled and tested on a PC.
 */
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "matrix.h"

void view_init(void);

/* A status update: state 'B' starting, 'O' ok, 'W' warning, 'E' error, 'D' stopping. */
void view_status(char state, int64_t requests, int64_t uploads, uint32_t now_ms);

/* Scrolls `text` once. */
void view_text(const char *text, size_t len, uint32_t now_ms);

/* When the last status update arrived (0 = never). */
uint32_t view_last_status_ms(void);

/* Draws the current view into `fb` (row-major brightness 0..7). */
void view_render(uint8_t fb[MATRIX_LEDS], uint32_t now_ms);
