// C17 6.7.6.2p4-p5, 6.5.3.4p2, 6.7.8p3, 6.5.6p8-p9, 6.9.1p10: the
// variable length array program of vla-runtime.c, at -O2.
//@ std: c17
//@ args: -O2
//@ expect: run
//@ stdout: 16 16 16 8 3 180005 780 12 1 199990000 ok
#include "Inputs/vla-runtime.h"
