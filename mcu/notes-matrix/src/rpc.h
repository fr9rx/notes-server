/*
 * MessagePack-RPC through arduino-router, implemented directly (no Arduino
 * Bridge library). Plain C: I/O goes through the callbacks, so it can be
 * tested on a PC.
 *
 * The router forwards calls and notifications for a method to the client that
 * registered it with `$/register`. This firmware registers:
 *
 *   notes/status   notification  [state, requests, uploads]
 *                  state: "B" starting, "O" ok, "W" warning, "E" error, "D" stopping
 *   notes/text     notification  [text]         scroll text once
 *   notes/hello    request       []  -> RPC_HELLO
 *
 * Messages: request [0, id, method, params], response [1, id, error, result],
 * notification [2, method, params], as one continuous MessagePack stream.
 */
#pragma once

#include <stddef.h>
#include <stdint.h>

#define RPC_HELLO "notes-matrix 3"

struct rpc_callbacks {
	void (*write)(const uint8_t *buf, size_t len);
	void (*status)(char state, int64_t requests, int64_t uploads);
	void (*text)(const char *text, size_t len);
	void (*log)(const char *msg);
};

void rpc_init(const struct rpc_callbacks *cb);

/* Sends `$/register` for every method (safe to repeat: re-registering is a no-op). */
void rpc_register(void);

/* Feeds received bytes. */
void rpc_feed(const uint8_t *buf, size_t len);
