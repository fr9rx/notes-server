# notes-matrix: native Zephyr firmware for the Arduino UNO Q's STM32U585.
# Build (from mcu/):  west build -b arduino_uno_q notes-matrix -d build
cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(notes_matrix LANGUAGES C)

target_sources(app PRIVATE
  src/main.c
  src/matrix.c
  src/link.c
  src/view.c
  src/mpack.c
  src/rpc.c
)
