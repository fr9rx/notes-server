/*
 * Views:
 *   waiting for the server  dim dot sweeping along the bottom row
 *   starting (B)            spinning comet
 *   ok (O)                  bar graph of requests/second over the last 13 s
 *                           (log scale, bright tops where images were
 *                           uploaded) and a heartbeat pixel top-right
 *   warning (W)             the same graph with the top row blinking
 *   error (E)               solid X
 *   server down             blinking X: no status line for 5 s (crash/hang)
 *   stopped (D)             dim dash: clean shutdown
 *   upload                  arrow flying up
 *   text                    scrolls across, then the normal view returns
 */

#include "view.h"

#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

#define W MATRIX_W
#define H MATRIX_H

#define DOWN_AFTER_MS   5000u
#define SCROLL_STEP_MS  70u
#define UPLOAD_ANIM_MS  900u
#define HEARTBEAT_MS    400u

static struct {
	char state; /* 0 = never heard from the server */
	uint32_t last_status_ms;
	uint16_t requests[W]; /* per second, oldest first */
	uint16_t uploads[W];
	bool upload_anim;
	uint32_t upload_anim_start;
	char text[64];
	int text_len;
	int text_x;
	bool text_active;
	uint32_t last_scroll_ms;
} s;

/* 3x5 font: 5 rows of 3 bits, bit 2 = left column. */
struct glyph {
	char c;
	uint8_t rows[5];
};

static const struct glyph font[] = {
	{'0', {7, 5, 5, 5, 7}}, {'1', {2, 6, 2, 2, 7}}, {'2', {7, 1, 7, 4, 7}},
	{'3', {7, 1, 7, 1, 7}}, {'4', {5, 5, 7, 1, 1}}, {'5', {7, 4, 7, 1, 7}},
	{'6', {7, 4, 7, 5, 7}}, {'7', {7, 1, 2, 2, 2}}, {'8', {7, 5, 7, 5, 7}},
	{'9', {7, 5, 7, 1, 7}}, {'A', {2, 5, 7, 5, 5}}, {'B', {6, 5, 6, 5, 6}},
	{'C', {3, 4, 4, 4, 3}}, {'D', {6, 5, 5, 5, 6}}, {'E', {7, 4, 6, 4, 7}},
	{'F', {7, 4, 6, 4, 4}}, {'G', {3, 4, 5, 5, 3}}, {'H', {5, 5, 7, 5, 5}},
	{'I', {7, 2, 2, 2, 7}}, {'J', {1, 1, 1, 5, 2}}, {'K', {5, 5, 6, 5, 5}},
	{'L', {4, 4, 4, 4, 7}}, {'M', {5, 7, 5, 5, 5}}, {'N', {6, 5, 5, 5, 5}},
	{'O', {2, 5, 5, 5, 2}}, {'P', {6, 5, 6, 4, 4}}, {'Q', {2, 5, 5, 6, 3}},
	{'R', {6, 5, 6, 5, 5}}, {'S', {3, 4, 2, 1, 6}}, {'T', {7, 2, 2, 2, 2}},
	{'U', {5, 5, 5, 5, 7}}, {'V', {5, 5, 5, 5, 2}}, {'W', {5, 5, 5, 7, 5}},
	{'X', {5, 5, 2, 5, 5}}, {'Y', {5, 5, 2, 2, 2}}, {'Z', {7, 1, 2, 4, 7}},
	{'.', {0, 0, 0, 0, 2}}, {':', {0, 2, 0, 2, 0}}, {'-', {0, 0, 7, 0, 0}},
	{'/', {1, 1, 2, 4, 4}}, {'!', {2, 2, 2, 0, 2}}, {'%', {5, 1, 2, 4, 5}},
};

static const uint8_t *glyph_rows(char c)
{
	static const uint8_t blank[5];

	for (size_t i = 0; i < sizeof(font) / sizeof(font[0]); i++) {
		if (font[i].c == c) {
			return font[i].rows;
		}
	}
	return blank;
}

static uint8_t *fb_;

static void px(int x, int y, uint8_t level)
{
	if (x >= 0 && x < W && y >= 0 && y < H && level > fb_[y * W + x]) {
		fb_[y * W + x] = level;
	}
}

static void draw_x(void)
{
	for (int i = 0; i < H; i++) {
		px(3 + i, i, MATRIX_MAX);
		px(10 - i, i, MATRIX_MAX);
	}
}

static void view_waiting(uint32_t now)
{
	int pos = (int)((now / 120) % (2 * (W - 1)));

	px(pos < W ? pos : 2 * (W - 1) - pos, H - 1, 2);
}

static void view_starting(uint32_t now)
{
	/* 12 points on a circle around the matrix centre. */
	static const int8_t ring[12][2] = {
		{6, 0}, {8, 1}, {9, 2}, {9, 4}, {9, 5}, {8, 6},
		{6, 7}, {4, 6}, {3, 5}, {3, 4}, {3, 2}, {4, 1},
	};
	int head = (int)((now / 70) % 12);

	for (int k = 0; k < 5; k++) {
		int i = (head - k + 12) % 12;

		px(ring[i][0], ring[i][1], (uint8_t)(MATRIX_MAX - k));
	}
}

static int bar_height(uint16_t n)
{
	int h = 0;

	if (n > 0) { /* log2 scale: 1, 2-3, 4-7, ... 64+ */
		h = 1;
		while (n > 1 && h < H - 1) {
			n >>= 1;
			h++;
		}
	}
	return h;
}

static void view_dashboard(uint32_t now, bool warning)
{
	for (int x = 0; x < W; x++) {
		int h = bar_height(s.requests[x]);

		for (int i = 0; i < h; i++) {
			px(x, H - 1 - i, 2);
		}
		if (s.uploads[x] > 0) {
			px(x, H - 1 - (h > 0 ? h - 1 : 0), MATRIX_MAX);
		}
	}
	if (warning && (now / 500) % 2 == 0) {
		for (int x = 0; x < W; x++) {
			px(x, 0, MATRIX_MAX);
		}
	}
	uint32_t since = now - s.last_status_ms;

	if (since < HEARTBEAT_MS) {
		px(W - 1, 0, (uint8_t)(MATRIX_MAX - since * MATRIX_MAX / HEARTBEAT_MS));
	}
}

static void view_upload(uint32_t now)
{
	static const uint8_t arrow[5] = {0x04, 0x0e, 0x15, 0x04, 0x04}; /* 5 wide */
	int top = H - (int)((now - s.upload_anim_start) * (H + 5) / UPLOAD_ANIM_MS);

	for (int r = 0; r < 5; r++) {
		for (int c = 0; c < 5; c++) {
			if (arrow[r] & (0x10 >> c)) {
				px(4 + c, top + r, MATRIX_MAX);
			}
		}
	}
}

static void draw_text(void)
{
	for (int i = 0; i < s.text_len; i++) {
		const uint8_t *g = glyph_rows(s.text[i]);
		int gx = s.text_x + i * 4;

		if (gx >= W || gx + 3 < 0) {
			continue;
		}
		for (int r = 0; r < 5; r++) {
			for (int c = 0; c < 3; c++) {
				if (g[r] & (4 >> c)) {
					px(gx + c, 1 + r, MATRIX_MAX);
				}
			}
		}
	}
}

static void push_history(uint16_t requests, uint16_t uploads)
{
	memmove(s.requests, s.requests + 1, (W - 1) * sizeof(s.requests[0]));
	memmove(s.uploads, s.uploads + 1, (W - 1) * sizeof(s.uploads[0]));
	s.requests[W - 1] = requests;
	s.uploads[W - 1] = uploads;
}

static uint16_t clamp_u16(int64_t v)
{
	return v < 0 ? 0 : v > 65535 ? 65535 : (uint16_t)v;
}

void view_init(void)
{
	memset(&s, 0, sizeof(s));
}

void view_status(char state, int64_t requests, int64_t uploads, uint32_t now)
{
	if (strchr("BOWED", state) == NULL || state == '\0') {
		return;
	}
	if (state == 'O' || state == 'W') {
		push_history(clamp_u16(requests), clamp_u16(uploads));
	}
	if (uploads > 0 && !s.upload_anim) {
		s.upload_anim = true;
		s.upload_anim_start = now;
	}
	s.state = state;
	s.last_status_ms = now;
}

void view_text(const char *text, size_t len, uint32_t now)
{
	if (len > sizeof(s.text) - 1) {
		len = sizeof(s.text) - 1;
	}
	memcpy(s.text, text, len);
	s.text[len] = '\0';
	s.text_len = (int)len;
	s.text_x = W;
	s.text_active = len > 0;
	s.last_scroll_ms = now;
}

uint32_t view_last_status_ms(void)
{
	return s.state ? s.last_status_ms : 0;
}

void view_render(uint8_t fb[MATRIX_LEDS], uint32_t now)
{
	fb_ = fb;
	memset(fb, 0, MATRIX_LEDS);

	while (s.text_active && now - s.last_scroll_ms >= SCROLL_STEP_MS) {
		s.last_scroll_ms += SCROLL_STEP_MS;
		if (--s.text_x < -s.text_len * 4) {
			s.text_active = false;
		}
	}
	if (s.upload_anim && now - s.upload_anim_start >= UPLOAD_ANIM_MS) {
		s.upload_anim = false;
	}

	bool down = s.state != 0 && s.state != 'D' && now - s.last_status_ms > DOWN_AFTER_MS;

	if (down) {
		if ((now / 500) % 2 == 0) {
			draw_x();
		}
	} else if (s.text_active) {
		draw_text();
	} else if (s.upload_anim) {
		view_upload(now);
	} else {
		switch (s.state) {
		case 0:
			view_waiting(now);
			break;
		case 'B':
			view_starting(now);
			break;
		case 'O':
			view_dashboard(now, false);
			break;
		case 'W':
			view_dashboard(now, true);
			break;
		case 'E':
			draw_x();
			break;
		case 'D':
			for (int x = 4; x <= 8; x++) {
				px(x, 3, 2);
				px(x, 4, 2);
			}
			break;
		}
	}
}
