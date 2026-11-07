// [basic.fundamental], [expr.call]/7: `long double` (AAPCS64 `fp128`,
// x86-64 `x86_fp80` in memory, X87 class), `__int128` and `_BitInt(128)`
// alone and inside classes (spec/landed/abi/WP-766.md, spec/abi.md
// §8.25). Compared with clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
typedef long double LD;
typedef __int128 I128;
typedef unsigned __int128 U128;
typedef _Complex long double CLD;
struct SLD { long double v; };
struct SLDI { long double v; int i; };
struct SI128 { __int128 v; };
struct SI128L { __int128 v; long l; };
struct SLI128 { long l; __int128 v; };
struct SDLD { double d; long double v; };
struct SFL { float f; long double v; };
ABI_SHAPE(LD)
ABI_SHAPE(I128)
ABI_SHAPE(U128)
ABI_SHAPE(CLD)
ABI_SHAPE(SLD)
ABI_SHAPE(SLDI)
ABI_SHAPE(SI128)
ABI_SHAPE(SI128L)
ABI_SHAPE(SLI128)
ABI_SHAPE(SDLD)
ABI_SHAPE(SFL)
