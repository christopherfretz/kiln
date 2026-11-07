// [class.prop], [expr.call]/7: trivially copyable aggregates of 1 to 33
// bytes, of chars, ints and longs (spec/landed/abi/WP-766.md): AAPCS64
// passes up to 16 bytes in registers (`i64`, `[2 x i64]`), more by
// reference; x86-64 classifies each eightbyte INTEGER and passes more
// than 16 bytes `byval` in memory, returning it `sret`. Compared with
// clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
struct P1 { char c[1]; };
struct P2 { char c[2]; };
struct P3 { char c[3]; };
struct P4 { char c[4]; };
struct P8 { char c[8]; };
struct P9 { char c[9]; };
struct P12 { char c[12]; };
struct P16 { char c[16]; };
struct P17 { char c[17]; };
struct P24 { char c[24]; };
struct P32 { char c[32]; };
struct P33 { char c[33]; };
struct S2 { short a; };
struct S3 { short a; char b; };
struct I1 { int a; };
struct I3 { int a, b, c; };
struct I5 { int a, b, c, d, e; };
struct L2 { long a, b; };
struct L3 { long a, b, c; };
struct L4 { long a, b, c, d; };
struct CL { char c; long l; };
struct LC { long l; char c; };
struct Ptr2 { void *a, *b; };
struct Nest { I1 a; S3 b; };
struct Der : I1 { short s; };
ABI_SHAPE(P1)
ABI_SHAPE(P2)
ABI_SHAPE(P3)
ABI_SHAPE(P4)
ABI_SHAPE(P8)
ABI_SHAPE(P9)
ABI_SHAPE(P12)
ABI_SHAPE(P16)
ABI_SHAPE(P17)
ABI_SHAPE(P24)
ABI_SHAPE(P32)
ABI_SHAPE(P33)
ABI_SHAPE(S2)
ABI_SHAPE(S3)
ABI_SHAPE(I1)
ABI_SHAPE(I3)
ABI_SHAPE(I5)
ABI_SHAPE(L2)
ABI_SHAPE(L3)
ABI_SHAPE(L4)
ABI_SHAPE(CL)
ABI_SHAPE(LC)
ABI_SHAPE(Ptr2)
ABI_SHAPE(Nest)
ABI_SHAPE_NP(Der)
