// [dcl.align], [expr.call]/7: over-aligned classes and members
// (`alignas(16)`, `alignas(32)`; spec/landed/abi/WP-766.md). AAPCS64
// classifies by the natural (unadjusted) alignment; x86-64 passes an
// over-aligned class in memory `byval align N` when it does not fit the
// eightbyte rules. Compared with clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
struct alignas(16) A16L { long a; };
struct alignas(16) A16LL { long a, b; };
struct alignas(32) A32C { char c; };
struct alignas(32) A32D4 { double a, b, c, d; };
struct alignas(16) A16F2 { float a, b; };
struct alignas(16) A16F4 { float a, b, c, d; };
struct M16 { alignas(16) long a; long b; };
struct M16C { char c; alignas(16) int i; };
struct M16F { alignas(16) float a; float b; };
struct M32 { alignas(32) int a; };
struct M8C { alignas(8) char c; };
struct Pk { char c; long l; } __attribute__((packed));
struct Pk2 { short s; int i; } __attribute__((packed, aligned(2)));
ABI_SHAPE(A16L)
ABI_SHAPE(A16LL)
ABI_SHAPE(A32C)
ABI_SHAPE(A32D4)
ABI_SHAPE(A16F2)
ABI_SHAPE(A16F4)
ABI_SHAPE(M16)
ABI_SHAPE(M16C)
ABI_SHAPE(M16F)
ABI_SHAPE(M32)
ABI_SHAPE(M8C)
ABI_SHAPE(Pk)
ABI_SHAPE(Pk2)
