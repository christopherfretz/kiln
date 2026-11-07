// [class.union], [expr.call]/7: unions across the call boundary
// (spec/landed/abi/WP-766.md): a union's classes merge per eightbyte on
// x86-64; on AAPCS64 a union of identical floating members is an HFA.
// Compared with clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
union UIF { int i; float f; };
union UFF { float a; float b; };
union UDL { double d; long l; };
union UC9 { char c[9]; };
union UFD { float f[2]; double d; };
union UF4 { float a[4]; struct { float x, y, z, w; } s; };
union UF3D { float f[3]; double d; };
union UBig { long a[3]; double d; };
union UE {};
union UV { float f[4]; __attribute__((vector_size(16))) float v; };
struct SU { UIF u; int x; };
struct SUF { UFF u; float x; };
ABI_SHAPE(UIF)
ABI_SHAPE(UFF)
ABI_SHAPE(UDL)
ABI_SHAPE(UC9)
ABI_SHAPE(UFD)
ABI_SHAPE(UF4)
ABI_SHAPE(UF3D)
ABI_SHAPE(UBig)
ABI_SHAPE(UE)
ABI_SHAPE(UV)
ABI_SHAPE(SU)
ABI_SHAPE(SUF)
