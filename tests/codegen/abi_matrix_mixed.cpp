// [expr.call]/7, SysV x86-64 psABI §3.2.3 (eightbyte classification,
// INTEGER and SSE merged per eightbyte): mixed integer/floating classes
// (spec/landed/abi/WP-766.md). AAPCS64 passes them as integer
// registers. Compared with clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
struct IF { int a; float b; };
struct FI { float a; int b; };
struct DI { double a; int b; };
struct ID { int a; double b; };
struct FFI { float a, b; int c; };
struct IIFF { int a, b; float c, d; };
struct CD { char a; double b; };
struct FDF { float a; double b; float c; };
struct DFF { double a; float b, c; };
struct LF { long a; float b; };
struct FL { float a; long b; };
struct DL { double a; long b; };
struct PD { void *p; double d; };
struct SF { short a; float b; };
struct IFIF { int a; float b; int c; float d; };
struct DDI { double a, b; int c; };
struct H8 { _Float16 a; int b; };
ABI_SHAPE(IF)
ABI_SHAPE(FI)
ABI_SHAPE(DI)
ABI_SHAPE(ID)
ABI_SHAPE(FFI)
ABI_SHAPE(IIFF)
ABI_SHAPE(CD)
ABI_SHAPE(FDF)
ABI_SHAPE(DFF)
ABI_SHAPE(LF)
ABI_SHAPE(FL)
ABI_SHAPE(DL)
ABI_SHAPE(PD)
ABI_SHAPE(SF)
ABI_SHAPE(IFIF)
ABI_SHAPE(DDI)
ABI_SHAPE(H8)
