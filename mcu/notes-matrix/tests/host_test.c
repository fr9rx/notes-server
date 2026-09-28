/*
 * PC-side tests for the protocol and view code (no Zephyr needed):
 *   zig cc -std=c11 -Wall -Wextra -Isrc tests/host_test.c src/mpack.c src/rpc.c src/view.c -o host_test && ./host_test
 */
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "mpack.h"
#include "rpc.h"
#include "view.h"

static uint8_t sent[1024];
static size_t sent_len;
static char got_state;
static int64_t got_requests, got_uploads;
static char got_text[64];
static char got_log[128];

static void cb_write(const uint8_t *buf, size_t len)
{
	memcpy(sent + sent_len, buf, len);
	sent_len += len;
}
static void cb_status(char state, int64_t requests, int64_t uploads)
{
	got_state = state;
	got_requests = requests;
	got_uploads = uploads;
}
static void cb_text(const char *text, size_t len)
{
	memcpy(got_text, text, len);
	got_text[len] = '\0';
}
static void cb_log(const char *msg)
{
	snprintf(got_log, sizeof(got_log), "%s", msg);
}

static const struct rpc_callbacks cbs = {cb_write, cb_status, cb_text, cb_log};

/* Bytes exactly as rmpv (the server) / the router encode them. */
static const uint8_t status_msg[] = {
	0x93, 0x02, 0xac, 'n', 'o', 't', 'e', 's', '/', 's', 't', 'a', 't', 'u', 's',
	0x93, 0xa1, 'O', 0xcd, 0x01, 0x2c, 0x03, /* ["O", 300, 3] */
};
static const uint8_t text_msg[] = {
	0x93, 0x02, 0xaa, 'n', 'o', 't', 'e', 's', '/', 't', 'e', 'x', 't',
	0x91, 0xa6, 'I', 'P', ' ', '1', '.', '2',
};
static const uint8_t hello_req[] = {
	0x94, 0x00, 0xcd, 0x04, 0xd2, 0xab, 'n', 'o', 't', 'e', 's', '/', 'h', 'e', 'l', 'l', 'o', 0x90,
};
static bool contains(const uint8_t *hay, size_t n, const char *needle)
{
	size_t k = strlen(needle);

	for (size_t i = 0; i + k <= n; i++) {
		if (memcmp(hay + i, needle, k) == 0) {
			return true;
		}
	}
	return false;
}

static void test_value_len(void)
{
	assert(mp_value_len(status_msg, sizeof(status_msg)) == (int)sizeof(status_msg));
	for (size_t n = 0; n < sizeof(status_msg); n++) {
		assert(mp_value_len(status_msg, n) == 0); /* every prefix is incomplete */
	}
	const uint8_t bad[] = {0xc1};
	assert(mp_value_len(bad, 1) < 0);
	const uint8_t map[] = {0x81, 0xa1, 'k', 0xcb, 0, 0, 0, 0, 0, 0, 0, 0}; /* {"k": 0.0} */
	assert(mp_value_len(map, sizeof(map)) == (int)sizeof(map));
	puts("mp_value_len ok");
}

static void test_writer_roundtrip(void)
{
	uint8_t buf[64];
	struct mp_writer w;
	struct mp_reader r;
	uint32_t n;
	int64_t v;
	const char *s;
	uint32_t len;

	mp_writer_init(&w, buf, sizeof(buf));
	mp_write_array(&w, 3);
	mp_write_uint(&w, 70000);
	mp_write_str(&w, "$/register");
	mp_write_nil(&w);
	assert(w.ok);
	mp_reader_init(&r, buf, w.len);
	assert(mp_read_array(&r, &n) && n == 3);
	assert(mp_read_int(&r, &v) && v == 70000);
	assert(mp_read_str(&r, &s, &len) && len == 10 && memcmp(s, "$/register", 10) == 0);
	assert(mp_read_nil(&r));
	assert(r.p == r.end);

	const uint8_t neg[] = {0xd1, 0xff, 0x38}; /* int16 -200 */
	mp_reader_init(&r, neg, sizeof(neg));
	assert(mp_read_int(&r, &v) && v == -200);

	mp_writer_init(&w, buf, 4);
	mp_write_str(&w, "too long");
	assert(!w.ok);
	puts("writer/reader ok");
}

static void test_rpc(void)
{
	rpc_init(&cbs);

	/* Registration: three requests the router understands. */
	sent_len = 0;
	rpc_register();
	int count = 0;
	for (size_t pos = 0; pos < sent_len; count++) {
		int m = mp_value_len(sent + pos, sent_len - pos);
		assert(m > 0);
		pos += (size_t)m;
	}
	assert(count == 3);
	assert(contains(sent, sent_len, "$/register") && contains(sent, sent_len, "notes/hello"));

	/* Status split across reads, with leading garbage (e.g. line noise at boot). */
	uint8_t stream[128];
	size_t sl = 0;
	stream[sl++] = 0xc1;
	stream[sl++] = 0xc1;
	memcpy(stream + sl, status_msg, sizeof(status_msg));
	sl += sizeof(status_msg);
	memcpy(stream + sl, text_msg, sizeof(text_msg));
	sl += sizeof(text_msg);
	rpc_feed(stream, 5);
	assert(got_state == 0);
	rpc_feed(stream + 5, sl - 5);
	assert(got_state == 'O' && got_requests == 300 && got_uploads == 3);
	assert(strcmp(got_text, "IP 1.2") == 0);

	/* hello request -> [1, 1234, nil, "notes-matrix 3"] */
	sent_len = 0;
	rpc_feed(hello_req, sizeof(hello_req));
	struct mp_reader r;
	uint32_t n;
	int64_t v;
	const char *s;
	uint32_t len;
	mp_reader_init(&r, sent, sent_len);
	assert(mp_read_array(&r, &n) && n == 4);
	assert(mp_read_int(&r, &v) && v == 1);
	assert(mp_read_int(&r, &v) && v == 1234);
	assert(mp_read_nil(&r));
	assert(mp_read_str(&r, &s, &len) && len == strlen(RPC_HELLO) && memcmp(s, RPC_HELLO, len) == 0);

	/* Router error array is logged. */
	uint8_t err[64];
	struct mp_writer w;
	mp_writer_init(&w, err, sizeof(err));
	mp_write_array(&w, 4);
	mp_write_uint(&w, 1);
	mp_write_uint(&w, 7);
	mp_write_array(&w, 2);
	mp_write_uint(&w, 5);
	mp_write_str(&w, "route already exists: notes/text");
	mp_write_nil(&w);
	rpc_feed(err, w.len);
	assert(strstr(got_log, "route already exists"));
	puts("rpc ok");
}

static void test_view(void)
{
	uint8_t fb[MATRIX_LEDS];

	view_init();
	view_render(fb, 1000); /* waiting: one dim dot on the bottom row */
	int lit = 0;
	for (int i = 0; i < MATRIX_LEDS; i++) {
		lit += fb[i] > 0;
		if (fb[i]) {
			assert(i / MATRIX_W == MATRIX_H - 1);
		}
	}
	assert(lit == 1);

	view_status('O', 100, 0, 2000);
	view_render(fb, 2000);
	/* 100 req/s -> log2 bar of 7 in the rightmost column (+ heartbeat pixel at top-right). */
	for (int y = 1; y < MATRIX_H; y++) {
		assert(fb[y * MATRIX_W + MATRIX_W - 1] > 0);
	}
	view_render(fb, 9000); /* 7 s without status: blinking X ("down") */
	int x_on = 0;
	for (int i = 0; i < MATRIX_LEDS; i++) {
		x_on += fb[i] == MATRIX_MAX;
	}
	assert(x_on == 16 || x_on == 0);
	view_status('D', 0, 0, 9100); /* stopped cleanly: never shows "down" */
	view_render(fb, 20000);
	assert(fb[3 * MATRIX_W + 6] > 0);
	puts("view ok");
}

int main(void)
{
	test_value_len();
	test_writer_roundtrip();
	test_rpc();
	test_view();
	puts("all host tests passed");
	return 0;
}
