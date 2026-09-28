/*
 * LED matrix driver for the Arduino UNO Q.
 *
 * The 104 LEDs are charlieplexed on PF0..PF10: each LED sits between two of
 * the lines, lit by driving one high and the other low while every other line
 * floats (input). Only one LED is on at a time; a TIM17 interrupt every 10 us
 * moves to the next one, so the whole matrix refreshes at ~960 Hz. Brightness
 * comes from lighting an LED on only some of the 8 scans of a cycle.
 *
 * The LED -> line-pair table and the register-level switching are from
 * Arduino's loader (ArduinoCore-zephyr loader/matrix.inc, Apache-2.0,
 * Copyright (c) Arduino s.r.l. and/or its affiliated companies).
 */

#include <string.h>

#include <soc.h>
#include <zephyr/device.h>
#include <zephyr/drivers/counter.h>
#include <zephyr/drivers/gpio.h>
#include <zephyr/kernel.h>

#include "matrix.h"

#define MATRIX_LINES 11

/* {high line, low line} for LED n, row-major (n = row * 13 + column). */
static const uint8_t pins[MATRIX_LEDS][2] = {
	{0, 1}, {1, 0}, {0, 2}, {2, 0}, {1, 2}, {2, 1}, {0, 3}, {3, 0}, {1, 3}, {3, 1},
	{2, 3}, {3, 2}, {0, 4}, {4, 0}, {1, 4}, {4, 1}, {2, 4}, {4, 2}, {3, 4}, {4, 3},
	{0, 5}, {5, 0}, {1, 5}, {5, 1}, {2, 5}, {5, 2}, {3, 5}, {5, 3}, {4, 5}, {5, 4},
	{0, 6}, {6, 0}, {1, 6}, {6, 1}, {2, 6}, {6, 2}, {3, 6}, {6, 3}, {4, 6}, {6, 4},
	{5, 6}, {6, 5}, {0, 7}, {7, 0}, {1, 7}, {7, 1}, {2, 7}, {7, 2}, {3, 7}, {7, 3},
	{4, 7}, {7, 4}, {5, 7}, {7, 5}, {6, 7}, {7, 6}, {0, 8}, {8, 0}, {1, 8}, {8, 1},
	{2, 8}, {8, 2}, {3, 8}, {8, 3}, {4, 8}, {8, 4}, {5, 8}, {8, 5}, {6, 8}, {8, 6},
	{7, 8}, {8, 7}, {0, 9}, {9, 0}, {1, 9}, {9, 1}, {2, 9}, {9, 2}, {3, 9}, {9, 3},
	{4, 9}, {9, 4}, {5, 9}, {9, 5}, {6, 9}, {9, 6}, {7, 9}, {9, 7}, {8, 9}, {9, 8},
	{0, 10}, {10, 0}, {1, 10}, {10, 1}, {2, 10}, {10, 2}, {3, 10}, {10, 3}, {4, 10}, {10, 4},
	{5, 10}, {10, 5}, {6, 10}, {10, 6},
};

/* Which of the 8 scans in a cycle light an LED of each brightness. */
static const uint8_t level_mask[MATRIX_MAX + 1] = {
	0x00, 0x01, 0x11, 0x49, 0x55, 0x5b, 0x77, 0xff,
};

/* MODER bits of PF0..PF10 (2 bits per pin); PF11..PF15 are left alone. */
#define LINES_MODER_MASK ((1U << (2 * MATRIX_LINES)) - 1)

static volatile uint8_t framebuffer[MATRIX_LEDS];

static inline void all_lines_float(void)
{
	GPIOF->MODER &= ~LINES_MODER_MASK;
}

static inline void light(int led)
{
	uint32_t hi = pins[led][0];
	uint32_t lo = pins[led][1];

	GPIOF->BSRR = BIT(hi) | BIT(lo + 16);
	GPIOF->MODER |= BIT(2 * hi) | BIT(2 * lo); /* 01 = output */
}

static void scan_isr(const struct device *dev, void *user_data)
{
	static int led;
	static uint8_t scan;

	ARG_UNUSED(dev);
	ARG_UNUSED(user_data);

	all_lines_float();
	if (level_mask[framebuffer[led] & MATRIX_MAX] & BIT(scan)) {
		light(led);
	}
	if (++led == MATRIX_LEDS) {
		led = 0;
		scan = (scan + 1) & 7;
	}
}

int matrix_init(void)
{
	const struct device *gpiof = DEVICE_DT_GET(DT_NODELABEL(gpiof));
	const struct device *timer = DEVICE_DT_GET(DT_NODELABEL(counter_matrix));

	if (!device_is_ready(gpiof) || !device_is_ready(timer)) {
		return -ENODEV;
	}
	/* Through the driver once so the port clock is on and pulls are off. */
	for (int i = 0; i < MATRIX_LINES; i++) {
		int err = gpio_pin_configure(gpiof, i, GPIO_INPUT);

		if (err) {
			return err;
		}
	}

	struct counter_top_cfg top = {
		.ticks = counter_us_to_ticks(timer, 10),
		.callback = scan_isr,
		.user_data = NULL,
		.flags = 0,
	};
	int err = counter_set_top_value(timer, &top);

	if (err) {
		return err;
	}
	return counter_start(timer);
}

void matrix_show(const uint8_t frame[MATRIX_LEDS])
{
	/* Byte copies: the ISR may see a frame half old/half new for one scan,
	 * which is invisible at ~960 Hz. */
	for (int i = 0; i < MATRIX_LEDS; i++) {
		framebuffer[i] = frame[i];
	}
}
