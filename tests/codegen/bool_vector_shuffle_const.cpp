// [WP-438] A `bool` ext-vector global initialized by a constant
// `__builtin_shufflevector` (clang ext_vector_type; [expr.const]; clang
// ConstExprEmitter) is a folded `<N x i1>` constant with no
// `__cxx_global_var_init`: same lane count, widening (`b4`, `b4` to 8
// lanes) and narrowing to 16 lanes. Under clang 19 the widening forms go
// through its GNU-vector bit cast and fold to clang 19's value
// (spec/oven.md "Bool vectors"); the comparison runs at the oracle's level.
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));

constexpr b4 A = {true, false, false, true};
constexpr b4 B = {false, true, true, true};
constexpr b8 C = {false, true, true, false, false, false, false, true};

b8 same = __builtin_shufflevector(C, C, 7, 6, 5, 4, 3, 2, 1, 0);
b8 wide = __builtin_shufflevector(A, B, 0, 1, 2, 3, 4, 5, 6, 7);
constexpr b8 kwide = __builtin_shufflevector(A, B, 7, 6, 5, 4, 3, 2, 1, 0);
b16 wider = __builtin_shufflevector(C, C, 1, 7, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

const void *p[] = {&kwide};
