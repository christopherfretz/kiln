// [class.prop]/9 (empty class), [dcl.attr.nouniqueaddr], [expr.call]/7:
// empty classes across the call boundary (spec/landed/abi/WP-766.md).
// AAPCS64: clang 20+ passes an empty class as `i64` (GCC's convention;
// clang 19: `i8`), returns it as `void`; a class whose only members are
// empty but not `[[no_unique_address]]` is not empty for the ABI. x86-64:
// an empty class takes no register and no IR argument. Compared with
// clang 19 and clang 22 on both targets. An x86-64 class whose first
// eightbyte holds only empty members is passed and returned from its
// second eightbyte (ABI offset 8; its `va_arg` read is not compared at
// -fclang-compat=19, where clang 19 reads the wrong eightbyte).
#include "Inputs/abi_matrix.h"
struct E {};
struct E2 { E a; };
struct EArr { E a[1]; };
struct ENua { [[no_unique_address]] E a; };
struct EBase : E {};
struct EBoth : E { [[no_unique_address]] E2 b; };
struct FE { float f; E e; };
struct FENua { float f; [[no_unique_address]] E e; };
struct DDNua { double a, b; [[no_unique_address]] E e; };
struct HfaEBase : E { float a, b, c, d; };
struct HfaNua5 { float a, b, c, d; [[no_unique_address]] E e; };
struct LE { long l; E e; };
struct O8 { [[no_unique_address]] E pad[8]; long b; };
struct O8I { E pad[8]; int b; };
ABI_SHAPE(E)
ABI_SHAPE(E2)
ABI_SHAPE(EArr)
ABI_SHAPE(ENua)
ABI_SHAPE(EBase)
ABI_SHAPE_NP(EBoth)
ABI_SHAPE(FE)
ABI_SHAPE(FENua)
ABI_SHAPE(DDNua)
ABI_SHAPE(HfaEBase)
ABI_SHAPE(HfaNua5)
ABI_SHAPE(LE)
ABI_SHAPE_NP(O8)
ABI_SHAPE_NP(O8I)
// Several empty arguments around scalars, in both orders.
void many(E, int, E, double, E);
E around(int a, E b, int c) { return b; }
void use_many() { many(E{}, 1, E{}, 2.0, E{}); around(1, E{}, 2); }
