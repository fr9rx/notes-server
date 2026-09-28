/*
 * notes-matrix: native Zephyr firmware for the Arduino UNO Q's STM32U585.
 *
 * Shows notes-server's status on the 8x13 LED matrix. Linux reaches it through
 * arduino-router: the router owns the UART to this chip and forwards
 * MessagePack-RPC calls for the methods registered here (see rpc.h).
 */

#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>

#include "link.h"
#include "matrix.h"
#include "rpc.h"
#include "view.h"

#define FRAME_MS 40 /* 25 fps */
/* Re-register while no status arrives: the router forgets registrations
 * when it restarts, and the server may simply not be running yet. */
#define REGISTER_EVERY_MS 10000
#define STATUS_FRESH_MS   3000

static void on_status(char state, int64_t requests, int64_t uploads)
{
	view_status(state, requests, uploads, k_uptime_get_32());
}

static void on_text(const char *text, size_t len)
{
	view_text(text, len, k_uptime_get_32());
}

static void on_log(const char *msg)
{
	printk("notes-matrix: %s\n", msg);
}

static const struct rpc_callbacks callbacks = {
	.write = link_write,
	.status = on_status,
	.text = on_text,
	.log = on_log,
};

int main(void)
{
	static uint8_t frame[MATRIX_LEDS];
	uint8_t rx[64];
	int err;

	view_init();
	rpc_init(&callbacks);
	err = matrix_init();
	if (err) {
		printk("notes-matrix: LED matrix init failed (%d)\n", err);
	}
	err = link_init();
	if (err) {
		printk("notes-matrix: UART init failed (%d)\n", err);
		return 0;
	}
	printk("notes-matrix: running (%s)\n", RPC_HELLO);

	int64_t next_frame = k_uptime_get();
	int64_t next_register = 0;

	for (;;) {
		int64_t now = k_uptime_get();
		uint32_t last_status = view_last_status_ms();
		bool fresh = last_status != 0 && (uint32_t)now - last_status < STATUS_FRESH_MS;

		if (now >= next_register && !fresh) {
			rpc_register();
			next_register = now + REGISTER_EVERY_MS;
		}

		int64_t wait = next_frame - now;
		size_t n = wait > 0 ? link_read(rx, sizeof(rx), K_MSEC(wait)) : 0;

		if (n > 0) {
			rpc_feed(rx, n);
			continue;
		}
		view_render(frame, k_uptime_get_32());
		matrix_show(frame);
		next_frame += FRAME_MS;
		if (next_frame < k_uptime_get()) {
			next_frame = k_uptime_get(); /* don't try to catch up after a stall */
		}
	}
	return 0;
}
