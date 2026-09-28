/*
 * Minimal MessagePack reader/writer: just what MessagePack-RPC through
 * arduino-router needs. Plain C, no allocation.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Length of the complete MessagePack value at the start of buf:
 * > 0 complete (bytes), 0 incomplete (need more data), < 0 malformed. */
int mp_value_len(const uint8_t *buf, size_t len);

struct mp_reader {
	const uint8_t *p;
	const uint8_t *end;
	bool ok; /* false after the first type mismatch or truncation */
};

void mp_reader_init(struct mp_reader *r, const uint8_t *buf, size_t len);
bool mp_read_array(struct mp_reader *r, uint32_t *count);
bool mp_read_int(struct mp_reader *r, int64_t *value);
/* Points *str into the buffer (not NUL-terminated). */
bool mp_read_str(struct mp_reader *r, const char **str, uint32_t *len);
/* Consumes a nil and returns true, or returns false (without error) if the next value is not nil. */
bool mp_read_nil(struct mp_reader *r);
bool mp_skip(struct mp_reader *r);

struct mp_writer {
	uint8_t *buf;
	size_t cap;
	size_t len;
	bool ok; /* false if the buffer overflowed */
};

void mp_writer_init(struct mp_writer *w, uint8_t *buf, size_t cap);
void mp_write_array(struct mp_writer *w, uint32_t count);
void mp_write_uint(struct mp_writer *w, uint64_t value);
void mp_write_str(struct mp_writer *w, const char *str);
void mp_write_nil(struct mp_writer *w);
