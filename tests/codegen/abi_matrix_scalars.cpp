// [basic.fundamental], [conv.prom], [expr.call]/7: scalar arguments and
// returns, with their `zeroext`/`signext` extension and `noundef`
// (spec/landed/abi/WP-766.md). Compared with clang 19 and clang 22 on
// AArch64 and x86-64.
#include "Inputs/abi_matrix.h"
typedef bool B;
typedef char C;
typedef signed char SC;
typedef unsigned char UC;
typedef short S;
typedef unsigned short US;
typedef wchar_t W;
typedef char8_t C8;
typedef char16_t C16;
typedef char32_t C32;
typedef int I;
typedef unsigned U;
typedef long L;
typedef unsigned long long ULL;
typedef float F;
typedef double D;
typedef void *P;
typedef decltype(nullptr) NP;
enum EnC : char { a1 };
enum class EnS : short { a2 };
enum EnU : unsigned char { a3 };
enum EnL : long { a4 };
struct K { void f(); int m; };
typedef int K::*DMP;
typedef void (K::*MFP)();
typedef _BitInt(7) BI7;
typedef unsigned _BitInt(17) UBI17;
typedef _BitInt(64) BI64;
ABI_SHAPE_FIXED(B)
ABI_SHAPE_FIXED(C)
ABI_SHAPE_FIXED(SC)
ABI_SHAPE_FIXED(UC)
ABI_SHAPE_FIXED(S)
ABI_SHAPE_FIXED(US)
ABI_SHAPE(W)
ABI_SHAPE_FIXED(C8)
ABI_SHAPE_FIXED(C16)
ABI_SHAPE(C32)
ABI_SHAPE(I)
ABI_SHAPE(U)
ABI_SHAPE(L)
ABI_SHAPE(ULL)
ABI_SHAPE_FIXED(F)
ABI_SHAPE(D)
ABI_SHAPE(P)
ABI_SHAPE_FIXED(NP)
ABI_SHAPE_FIXED(EnC)
ABI_SHAPE_FIXED(EnS)
ABI_SHAPE_FIXED(EnU)
ABI_SHAPE(EnL)
ABI_SHAPE(DMP)
ABI_SHAPE(MFP)
ABI_SHAPE_FIXED(BI7)
ABI_SHAPE_FIXED(UBI17)
ABI_SHAPE(BI64)
// Promoted variadic arguments ([expr.call]/12).
void promo(B b, C c, S s, F f, EnC e, US u) { abi_sink(0, b, c, s, f, e, u); }
