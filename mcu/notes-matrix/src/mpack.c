#include "mpack.h"

#include <string.h>

#define MAX_DEPTH 8

static uint32_t be(const uint8_t *p, int n)
{
	uint32_t v = 0;

	for (int i = 0; i < n; i++) {
		v = (v << 8) | p[i];
	}
	return v;
}

/* Scans one value; returns its length, 0 if incomplete, -1 if malformed. */
static long scan(const uint8_t *p, size_t len, int depth)
{
	if (depth > MAX_DEPTH) {
		return -1;
	}
	if (len < 1) {
		return 0;
	}

	uint8_t t = p[0];
	size_t head = 1;     /* bytes before the payload / children */
	uint64_t payload = 0; /* raw bytes after the head */
	uint32_t children = 0; /* nested values after the head */

#define NEED(n) \
	do { \
		if (len < (size_t)(n)) { \
			return 0; \
		} \
	} while (0)

	if (t <= 0x7f || t >= 0xe0 || t == 0xc0 || t == 0xc2 || t == 0xc3) {
		/* fixint, nil, bool */
	} else if (t >= 0x80 && t <= 0x8f) {
		children = 2u * (t & 0x0f);
	} else if (t >= 0x90 && t <= 0x9f) {
		children = t & 0x0f;
	} else if (t >= 0xa0 && t <= 0xbf) {
		payload = t & 0x1f;
	} else {
		switch (t) {
		case 0xc4: case 0xd9: NEED(2); head = 2; payload = p[1]; break;          /* bin8, str8 */
		case 0xc5: case 0xda: NEED(3); head = 3; payload = be(p + 1, 2); break;  /* bin16, str16 */
		case 0xc6: case 0xdb: NEED(5); head = 5; payload = be(p + 1, 4); break;  /* bin32, str32 */
		case 0xc7: NEED(2); head = 3; payload = p[1]; break;                     /* ext8 */
		case 0xc8: NEED(3); head = 4; payload = be(p + 1, 2); break;             /* ext16 */
		case 0xc9: NEED(5); head = 6; payload = be(p + 1, 4); break;             /* ext32 */
		case 0xca: payload = 4; break;                                           /* float32 */
		case 0xcb: payload = 8; break;                                           /* float64 */
		case 0xcc: case 0xd0: payload = 1; break;                                /* (u)int8 */
		case 0xcd: case 0xd1: payload = 2; break;
		case 0xce: case 0xd2: payload = 4; break;
		case 0xcf: case 0xd3: payload = 8; break;
		case 0xd4: payload = 2; break;                                           /* fixext1..16 */
		case 0xd5: payload = 3; break;
		case 0xd6: payload = 5; break;
		case 0xd7: payload = 9; break;
		case 0xd8: payload = 17; break;
		case 0xdc: NEED(3); head = 3; children = be(p + 1, 2); break;            /* array16 */
		case 0xdd: NEED(5); head = 5; children = be(p + 1, 4); break;            /* array32 */
		case 0xde: NEED(3); head = 3; children = 2u * be(p + 1, 2); break;       /* map16 */
		case 0xdf: NEED(5); head = 5; children = 2u * be(p + 1, 4); break;       /* map32 */
		default: return -1;                                                      /* 0xc1 */
		}
	}
#undef NEED

	if (payload > 0xffff) {
		return -1; /* nothing this firmware accepts is that big */
	}
	size_t total = head + (size_t)payload;

	if (len < total) {
		return 0;
	}
	for (uint32_t i = 0; i < children; i++) {
		long n = scan(p + total, len - total, depth + 1);

		if (n <= 0) {
			return n;
		}
		total += (size_t)n;
	}
	return (long)total;
}

int mp_value_len(const uint8_t *buf, size_t len)
{
	long n = scan(buf, len, 0);

	return n > 0x7fff ? -1 : (int)n;
}

void mp_reader_init(struct mp_reader *r, const uint8_t *buf, size_t len)
{
	r->p = buf;
	r->end = buf + len;
	r->ok = true;
}

static bool have(struct mp_reader *r, size_t n)
{
	if (!r->ok || (size_t)(r->end - r->p) < n) {
		r->ok = false;
	}
	return r->ok;
}

bool mp_read_array(struct mp_reader *r, uint32_t *count)
{
	if (!have(r, 1)) {
		return false;
	}
	uint8_t t = *r->p;

	if (t >= 0x90 && t <= 0x9f) {
		*count = t & 0x0f;
		r->p += 1;
	} else if (t == 0xdc && have(r, 3)) {
		*count = be(r->p + 1, 2);
		r->p += 3;
	} else if (t == 0xdd && have(r, 5)) {
		*count = be(r->p + 1, 4);
		r->p += 5;
	} else {
		r->ok = false;
	}
	return r->ok;
}

bool mp_read_int(struct mp_reader *r, int64_t *value)
{
	if (!have(r, 1)) {
		return false;
	}
	uint8_t t = *r->p;
	int n;

	if (t <= 0x7f) {
		*value = t;
		r->p += 1;
		return true;
	}
	if (t >= 0xe0) {
		*value = (int8_t)t;
		r->p += 1;
		return true;
	}
	switch (t) {
	case 0xcc: case 0xd0: n = 1; break;
	case 0xcd: case 0xd1: n = 2; break;
	case 0xce: case 0xd2: n = 4; break;
	case 0xcf: case 0xd3: n = 8; break;
	default:
		r->ok = false;
		return false;
	}
	if (!have(r, 1 + (size_t)n)) {
		return false;
	}
	uint64_t u = 0;

	for (int i = 0; i < n; i++) {
		u = (u << 8) | r->p[1 + i];
	}
	if (t >= 0xd0) { /* signed: sign-extend from n bytes */
		int shift = 64 - 8 * n;

		*value = (int64_t)(u << shift) >> shift;
	} else {
		*value = (int64_t)u;
	}
	r->p += 1 + n;
	return true;
}

bool mp_read_str(struct mp_reader *r, const char **str, uint32_t *len)
{
	if (!have(r, 1)) {
		return false;
	}
	uint8_t t = *r->p;
	size_t head;

	if (t >= 0xa0 && t <= 0xbf) {
		head = 1;
		*len = t & 0x1f;
	} else if (t == 0xd9 && have(r, 2)) {
		head = 2;
		*len = r->p[1];
	} else if (t == 0xda && have(r, 3)) {
		head = 3;
		*len = be(r->p + 1, 2);
	} else {
		r->ok = false;
		return false;
	}
	if (!have(r, head + *len)) {
		return false;
	}
	*str = (const char *)r->p + head;
	r->p += head + *len;
	return true;
}

bool mp_read_nil(struct mp_reader *r)
{
	if (have(r, 1) && *r->p == 0xc0) {
		r->p += 1;
		return true;
	}
	return false;
}

bool mp_skip(struct mp_reader *r)
{
	if (!r->ok) {
		return false;
	}
	long n = scan(r->p, (size_t)(r->end - r->p), 0);

	if (n <= 0) {
		r->ok = false;
		return false;
	}
	r->p += n;
	return true;
}

void mp_writer_init(struct mp_writer *w, uint8_t *buf, size_t cap)
{
	w->buf = buf;
	w->cap = cap;
	w->len = 0;
	w->ok = true;
}

static void put(struct mp_writer *w, const void *data, size_t n)
{
	if (!w->ok || w->cap - w->len < n) {
		w->ok = false;
		return;
	}
	memcpy(w->buf + w->len, data, n);
	w->len += n;
}

static void put_be(struct mp_writer *w, uint8_t type, uint64_t v, int n)
{
	uint8_t tmp[9];

	tmp[0] = type;
	for (int i = 0; i < n; i++) {
		tmp[n - i] = (uint8_t)(v >> (8 * i));
	}
	put(w, tmp, 1 + (size_t)n);
}

void mp_write_array(struct mp_writer *w, uint32_t count)
{
	if (count <= 15) {
		uint8_t t = 0x90 | (uint8_t)count;

		put(w, &t, 1);
	} else if (count <= 0xffff) {
		put_be(w, 0xdc, count, 2);
	} else {
		put_be(w, 0xdd, count, 4);
	}
}

void mp_write_uint(struct mp_writer *w, uint64_t v)
{
	if (v <= 0x7f) {
		uint8_t t = (uint8_t)v;

		put(w, &t, 1);
	} else if (v <= 0xff) {
		put_be(w, 0xcc, v, 1);
	} else if (v <= 0xffff) {
		put_be(w, 0xcd, v, 2);
	} else if (v <= 0xffffffffu) {
		put_be(w, 0xce, v, 4);
	} else {
		put_be(w, 0xcf, v, 8);
	}
}

void mp_write_str(struct mp_writer *w, const char *str)
{
	size_t n = strlen(str);

	if (n <= 31) {
		uint8_t t = 0xa0 | (uint8_t)n;

		put(w, &t, 1);
	} else if (n <= 0xff) {
		put_be(w, 0xd9, n, 1);
	} else {
		put_be(w, 0xda, n, 2);
	}
	put(w, str, n);
}

void mp_write_nil(struct mp_writer *w)
{
	uint8_t t = 0xc0;

	put(w, &t, 1);
}
