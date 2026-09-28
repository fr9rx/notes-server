/*
 * UART to Linux: LPUART1 <-> /dev/ttyHS1, which arduino-router owns on the
 * Linux side. Carries a MessagePack-RPC byte stream (see rpc.h).
 */
#pragma once

#include <stddef.h>
#include <stdint.h>
#include <zephyr/kernel.h>

/* Starts interrupt-driven reception. Returns 0 or -errno. */
int link_init(void);

/* Waits up to `timeout` for received bytes; returns how many were copied (0 on timeout). */
size_t link_read(uint8_t *buf, size_t cap, k_timeout_t timeout);

/* Sends bytes (blocking). */
void link_write(const uint8_t *buf, size_t len);
