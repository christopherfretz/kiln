// [expr.call]/7, AAPCS64 §5.9.5 (homogeneous aggregates): HFA/HVA
// candidates of float, double, long double, `_Float16`, `__bf16`, `__fp16`
// and 64/128-bit vectors, with 1 to 5 members, nested arrays, nested
// classes and bases (spec/landed/abi/WP-766.md). AAPCS64 passes up to
// four members in SIMD registers as `[N x T] alignstack`; x86-64
// classifies the eightbytes SSE. Compared with clang 19 and clang 22 on
// both targets.
#include "Inputs/abi_matrix.h"
typedef float V2F __attribute__((vector_size(8)));
typedef float V4F __attribute__((vector_size(16)));
typedef int V4I __attribute__((vector_size(16)));
typedef double V2D __attribute__((vector_size(16)));
struct F1 { float a; };
struct F2 { float a, b; };
struct F3 { float a, b, c; };
struct F4 { float a, b, c, d; };
struct F5 { float a, b, c, d, e; };
struct D1 { double a; };
struct D2 { double a, b; };
struct D3 { double a, b, c; };
struct D4 { double a, b, c, d; };
struct D5 { double a, b, c, d, e; };
struct LD2 { long double a, b; };
struct H2 { _Float16 a, b; };
struct H3 { _Float16 a, b, c; };
struct H4 { _Float16 a, b, c, d; };
struct H5 { _Float16 a, b, c, d, e; };
struct BF3 { __bf16 a, b, c; };
struct BF4 { __bf16 a, b, c, d; };
struct BF5 { __bf16 a, b, c, d, e; };
struct FP3 { __fp16 a, b, c; };
struct FP5 { __fp16 a, b, c, d, e; };
struct V1 { V4F a; };
struct V2 { V4F a, b; };
struct V4 { V4F a, b, c, d; };
struct V5 { V4F a, b, c, d, e; };
struct VI2 { V4I a; V4F b; };
struct VD2 { V2D a, b; };
struct VS3 { V2F a, b, c; };
struct VMix { V2F a; V4F b; };
struct FArr22 { float a[2][2]; };
struct FArr23 { float a[2]; float b[3]; };
struct DArr { double a[1][3]; };
struct FNest { F2 a; float b[2]; };
struct FBase : F2 { float c; };
struct FD { float a; double b; };
struct CF2 { _Complex float a, b; };
struct CD1 { _Complex double a; };
ABI_SHAPE(F1)
ABI_SHAPE(F2)
ABI_SHAPE(F3)
ABI_SHAPE(F4)
ABI_SHAPE(F5)
ABI_SHAPE(D1)
ABI_SHAPE(D2)
ABI_SHAPE(D3)
ABI_SHAPE(D4)
ABI_SHAPE(D5)
ABI_SHAPE(LD2)
ABI_SHAPE(H2)
ABI_SHAPE(H3)
ABI_SHAPE(H4)
ABI_SHAPE(H5)
ABI_SHAPE(BF3)
ABI_SHAPE(BF4)
ABI_SHAPE(BF5)
ABI_SHAPE(FP3)
ABI_SHAPE(FP5)
ABI_SHAPE(V1)
ABI_SHAPE(V2)
ABI_SHAPE(V4)
ABI_SHAPE(V5)
ABI_SHAPE(VI2)
ABI_SHAPE(VD2)
ABI_SHAPE(VS3)
ABI_SHAPE(VMix)
ABI_SHAPE(FArr22)
ABI_SHAPE(FArr23)
ABI_SHAPE(DArr)
ABI_SHAPE(FNest)
ABI_SHAPE_NP(FBase)
ABI_SHAPE(FD)
ABI_SHAPE(CF2)
ABI_SHAPE(CD1)
// Bare vectors.
ABI_SHAPE(V2F)
ABI_SHAPE(V4F)
ABI_SHAPE(V2D)
