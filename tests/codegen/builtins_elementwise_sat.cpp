// WP-432: __builtin_elementwise_add_sat / _sub_sat lower to
// `llvm.{s,u}{add,sub}.sat` by element signedness (clang CGBuiltin
// BI__builtin_elementwise_{add,sub}_sat). Same-type operands only: clang 19
// applies the elementwise usual conversions, clang 22 requires equal types
// (builtins_elementwise_sat_22.cpp covers folding, bool and enums).

typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned char v4u8 __attribute__((ext_vector_type(4)));

int run(int x, int y) { return __builtin_elementwise_add_sat(x, y); }
unsigned run2(unsigned x, unsigned y) { return __builtin_elementwise_sub_sat(x, y); }
v4i run3(v4i x, v4i y) { return __builtin_elementwise_sub_sat(x, y); }
v4u8 run4(v4u8 x, v4u8 y) { return __builtin_elementwise_add_sat(x, y); }
long long run5(long long x, long long y) { return __builtin_elementwise_add_sat(x, y); }
unsigned short run6(unsigned short x, unsigned short y) { return __builtin_elementwise_sub_sat(x, y); }
