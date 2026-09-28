#include "rpc.h"

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "mpack.h"

enum { REQUEST = 0, RESPONSE = 1, NOTIFICATION = 2 };

static const char *const methods[] = {"notes/status", "notes/text", "notes/hello"};

static const struct rpc_callbacks *cb;
static uint8_t rx[512];
static size_t rx_len;
static uint32_t next_id = 1;

void rpc_init(const struct rpc_callbacks *callbacks)
{
	cb = callbacks;
	rx_len = 0;
}

static void send(struct mp_writer *w)
{
	if (w->ok) {
		cb->write(w->buf, w->len);
	}
}

void rpc_register(void)
{
	uint8_t buf[64];
	struct mp_writer w;

	for (size_t i = 0; i < sizeof(methods) / sizeof(methods[0]); i++) {
		mp_writer_init(&w, buf, sizeof(buf));
		mp_write_array(&w, 4);
		mp_write_uint(&w, REQUEST);
		mp_write_uint(&w, next_id++);
		mp_write_str(&w, "$/register");
		mp_write_array(&w, 1);
		mp_write_str(&w, methods[i]);
		send(&w);
	}
}

static void respond(uint64_t id, const char *error, const char *result)
{
	uint8_t buf[96];
	struct mp_writer w;

	mp_writer_init(&w, buf, sizeof(buf));
	mp_write_array(&w, 4);
	mp_write_uint(&w, RESPONSE);
	mp_write_uint(&w, id);
	if (error) {
		mp_write_str(&w, error);
	} else {
		mp_write_nil(&w);
	}
	if (result) {
		mp_write_str(&w, result);
	} else {
		mp_write_nil(&w);
	}
	send(&w);
}

static bool method_is(const char *m, uint32_t len, const char *name)
{
	return len == strlen(name) && memcmp(m, name, len) == 0;
}

static void on_status(struct mp_reader *r)
{
	uint32_t n;
	const char *state;
	uint32_t state_len;
	int64_t requests, uploads;

	if (mp_read_array(r, &n) && n == 3 && mp_read_str(r, &state, &state_len) && state_len == 1 &&
	    mp_read_int(r, &requests) && mp_read_int(r, &uploads)) {
		cb->status(state[0], requests, uploads);
	}
}

static void on_text(struct mp_reader *r)
{
	uint32_t n;
	const char *text;
	uint32_t len;

	if (mp_read_array(r, &n) && n == 1 && mp_read_str(r, &text, &len)) {
		cb->text(text, len);
	}
}

/* One complete message of `len` bytes. */
static void dispatch(const uint8_t *msg, size_t len)
{
	struct mp_reader r;
	uint32_t n;
	int64_t type, id;
	const char *method;
	uint32_t method_len;

	mp_reader_init(&r, msg, len);
	if (!mp_read_array(&r, &n) || !mp_read_int(&r, &type)) {
		return;
	}
	if (type == NOTIFICATION && n == 3 && mp_read_str(&r, &method, &method_len)) {
		if (method_is(method, method_len, "notes/status")) {
			on_status(&r);
		} else if (method_is(method, method_len, "notes/text")) {
			on_text(&r);
		}
	} else if (type == REQUEST && n == 4 && mp_read_int(&r, &id) &&
		   mp_read_str(&r, &method, &method_len)) {
		if (method_is(method, method_len, "notes/hello")) {
			respond((uint64_t)id, NULL, RPC_HELLO);
		} else {
			respond((uint64_t)id, "method not supported by notes-matrix", NULL);
		}
	} else if (type == RESPONSE && n == 4 && mp_read_int(&r, &id)) {
		/* Replies to our $/register calls: only errors are interesting. */
		if (!mp_read_nil(&r) && cb->log) {
			/* The router's errors are [code, message]; others may send a string. */
			struct mp_reader as_array = r;
			const char *err = NULL;
			uint32_t err_len = 0, count;
			int64_t code;
			char line[96];

			if (mp_read_array(&as_array, &count) && count == 2 &&
			    mp_read_int(&as_array, &code) && mp_read_str(&as_array, &err, &err_len)) {
				/* err set */
			} else if (!mp_read_str(&r, &err, &err_len)) {
				err = NULL;
			}
			if (err) {
				snprintf(line, sizeof(line), "router error: %.*s", (int)err_len, err);
			} else {
				snprintf(line, sizeof(line), "router error for request %lld", (long long)id);
			}
			cb->log(line);
		}
	}
}

void rpc_feed(const uint8_t *buf, size_t len)
{
	while (len > 0) {
		size_t n = sizeof(rx) - rx_len;

		if (n > len) {
			n = len;
		}
		memcpy(rx + rx_len, buf, n);
		rx_len += n;
		buf += n;
		len -= n;

		/* Consume every complete message; drop bytes that can't start one. */
		size_t pos = 0;

		while (pos < rx_len) {
			int m = mp_value_len(rx + pos, rx_len - pos);

			if (m > 0) {
				dispatch(rx + pos, (size_t)m);
				pos += (size_t)m;
			} else if (m < 0) {
				pos += 1; /* garbage: resynchronise */
			} else if (pos == 0 && rx_len == sizeof(rx)) {
				pos = rx_len; /* one message bigger than our buffer: discard */
			} else {
				break; /* incomplete: wait for more bytes */
			}
		}
		memmove(rx, rx + pos, rx_len - pos);
		rx_len -= pos;
	}
}
