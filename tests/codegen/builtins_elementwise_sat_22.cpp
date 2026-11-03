//@ min-oracle: 22
// WP-432: clang 22 constant-evaluates __builtin_elementwise_add_sat /
// _sub_sat, so these initializers are constant (no dynamic init) and
// `use` folds its call; bool and enum operands keep their type (bool ->
// i1 uadd.sat). Under clang 19 the same source emits dynamic initializers
// (constant cases: conformance ext.gnu.builtins/elementwise-sat-constexpr*).

typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned char v4u8 __attribute__((ext_vector_type(4)));
enum E { e0, e1, e2 };
const int IMAX = 2147483647;

int g = __builtin_elementwise_add_sat(IMAX, IMAX);
unsigned g2 = __builtin_elementwise_sub_sat(3u, 4u);
signed char g3 = __builtin_elementwise_add_sat((signed char)100, (signed char)100);
v4u8 g4 = __builtin_elementwise_add_sat((v4u8){250, 5, 0, 128}, (v4u8){10, 10, 1, 128});
int use() { return __builtin_elementwise_add_sat(1, 2); }
bool run5(bool x, bool y) { return __builtin_elementwise_add_sat(x, y); }
_BitInt(37) run6(_BitInt(37) x, _BitInt(37) y) { return __builtin_elementwise_sub_sat(x, y); }
E run7(E x) { return (E)__builtin_elementwise_add_sat(x, x); }
