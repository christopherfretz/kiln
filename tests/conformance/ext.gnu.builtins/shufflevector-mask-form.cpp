// __builtin_shufflevector (clang 19 BuiltinShuffleVector, ExprConstant):
// with exactly two arguments the second is an integer vector mask with the
// same element count, and each mask element (modulo the next power of two)
// selects an element of the first vector. A -1 index means "undefined" at
// run time, but in a constant expression it is an error
// (err_shufflevector_minus_one_is_undefined_behavior_constexpr, an error
// carried as an evaluation note).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:14 error "constexpr variable 'c2' must be initialized by a constant expression"
//@ diag: 22:19 error "index for __builtin_shufflevector not within the bounds of the input vectors; index of -1 found at position 0 is not permitted in a constexpr context"
//@ diag: 23:27 error "first two arguments to '__builtin_shufflevector' must have the same type"
//@ diag: 24:27 error "first two arguments to '__builtin_shufflevector' must have the same type"
typedef int v4 __attribute__((vector_size(16)));
typedef unsigned u4 __attribute__((vector_size(16)));
typedef int v2 __attribute__((vector_size(8)));
typedef float f4 __attribute__((vector_size(16)));
v4 a(v4 x, v4 y) { return __builtin_shufflevector(x, y, -1, 1, 2, 7); }
v4 b(v4 x, u4 m) { return __builtin_shufflevector(x, m); }
constexpr v4 cx = {1, 2, 3, 4};
constexpr v4 c1 = __builtin_shufflevector(cx, cx, 3, 2, 1, 0);
constexpr v4 c2 = __builtin_shufflevector(cx, cx, -1, 2, 1, 0);
v2 d(v4 x, v2 m) { return __builtin_shufflevector(x, m); }
v4 e(v4 x, f4 m) { return __builtin_shufflevector(x, m); }
