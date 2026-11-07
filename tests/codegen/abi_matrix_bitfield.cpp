// [class.bit], [expr.call]/7: bit-field classes, a zero-width bit-field
// between floating members (which breaks an AAPCS64 HFA only in C, not
// C++; clang `isHomogeneousAggregate` skips it) and unnamed bit-fields
// (spec/landed/abi/WP-766.md). Compared with clang 19 and clang 22 on
// both targets.
#include "Inputs/abi_matrix.h"
struct B1 { int a : 3, b : 5; };
struct B2 { int a : 1; int : 0; int b : 1; };
struct B3 { long a : 40; int b : 20; };
struct B4 { char a : 1; float f; };
struct B5 { unsigned a : 31; int : 0; float f; };
struct B6 { float a; int : 0; float b; };
struct B7 { float a; int : 3; float b; };
struct B8 { long a : 64; long b : 1; long c : 63; };
struct B9 { char a; int : 0; char b; };
struct B10 { unsigned long long a : 1; double d; };
struct B11 { int : 4; };
ABI_SHAPE(B1)
ABI_SHAPE(B2)
ABI_SHAPE(B3)
ABI_SHAPE(B4)
ABI_SHAPE(B5)
ABI_SHAPE(B6)
ABI_SHAPE(B7)
ABI_SHAPE(B8)
ABI_SHAPE(B9)
ABI_SHAPE(B10)
ABI_SHAPE(B11)
