// C17 6.5.2.5p4-p5, p15: a compound literal is an lvalue naming an unnamed
// object, static at file scope and automatic (to the end of its block)
// inside a function; its address is taken, it is assigned to and its
// members are written, and one literal names one object per block entry.
// The program of compound-literal-lvalue.c, at -O2.
//@ std: c17
//@ args: -O2
//@ expect: run
//@ stdout: 1 7 12 1 8 | 8 10 ok 1 42 16 | 8 12
#include "Inputs/compound-literal-lvalue.h"
