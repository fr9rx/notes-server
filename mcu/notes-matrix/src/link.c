#include <zephyr/device.h>
#include <zephyr/drivers/uart.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/ring_buffer.h>

#include "link.h"

static const struct device *const uart = DEVICE_DT_GET(DT_NODELABEL(lpuart1));

/* Bytes from the RX interrupt to the main loop. Overflow drops bytes; the
 * MessagePack parser resynchronises on the next valid message. */
RING_BUF_DECLARE(rx_ring, 1024);
K_SEM_DEFINE(rx_ready, 0, 1);

static void uart_isr(const struct device *dev, void *user_data)
{
	uint8_t buf[32];

	ARG_UNUSED(user_data);
	if (!uart_irq_update(dev)) {
		return;
	}
	while (uart_irq_rx_ready(dev)) {
		int n = uart_fifo_read(dev, buf, sizeof(buf));

		if (n <= 0) {
			break;
		}
		ring_buf_put(&rx_ring, buf, (uint32_t)n);
		k_sem_give(&rx_ready);
	}
}

int link_init(void)
{
	if (!device_is_ready(uart)) {
		return -ENODEV;
	}
	int err = uart_irq_callback_user_data_set(uart, uart_isr, NULL);

	if (err) {
		return err;
	}
	uart_irq_rx_enable(uart);
	return 0;
}

size_t link_read(uint8_t *buf, size_t cap, k_timeout_t timeout)
{
	size_t n = ring_buf_get(&rx_ring, buf, (uint32_t)cap);

	if (n == 0 && k_sem_take(&rx_ready, timeout) == 0) {
		n = ring_buf_get(&rx_ring, buf, (uint32_t)cap);
	}
	return n;
}

void link_write(const uint8_t *buf, size_t len)
{
	for (size_t i = 0; i < len; i++) {
		uart_poll_out(uart, buf[i]);
	}
}
