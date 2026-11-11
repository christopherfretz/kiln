// C17 6.7.6.2p4-p5, 6.5.3.4p2, 6.7.8p3, 6.5.6p8-p9, 6.9.1p10: variable
// length arrays at -O0 -- runtime sizeof, a VLA typedef evaluated once,
// pointer-to-VLA arithmetic, VLA parameters, a cast to a pointer to VLA,
// and the stack restored at block exit and on a jump out of the scope
// (20000 iterations of a 64 KiB VLA). vla-runtime-o2.c runs it at -O2.
//@ std: c17
//@ expect: run
//@ stdout: 16 16 16 8 3 180005 780 12 1 199990000 ok
#include "Inputs/vla-runtime.h"
