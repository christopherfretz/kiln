// [WP-490] `__builtin_convertvector` of a `const` vector variable inside a
// function folds as in clang ([expr.const]; clang
// ScalarExprEmitter::VisitConvertVectorExpr visits the operand as a
// scalar, `tryEmitAsConstant` folds the variable, and the IRBuilder's
// ConstantFolder folds the lane-wise `icmp`/`fcmp`/cast): to and from a
// `bool` ext vector, from integer, `float`, `half` and `bool` sources
// (a NaN lane is `true`, `-0.0` is `false`), a non-constexpr `const`
// local, a static data member and a variable in a function template.
// The folded `bool` vector is stored as clang stores it: `bitcast` of the
// widened `<8 x i1>` constant (its padding lanes `undef`) to `i8`.
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef int i4 __attribute__((ext_vector_type(4)));
typedef unsigned u4 __attribute__((ext_vector_type(4)));
typedef long long l4 __attribute__((ext_vector_type(4)));
typedef float f4 __attribute__((ext_vector_type(4)));
typedef double d4 __attribute__((ext_vector_type(4)));
typedef _Float16 h4 __attribute__((ext_vector_type(4)));
typedef unsigned char uc4 __attribute__((ext_vector_type(4)));

constexpr i4 K = {1, 0, -3, 0};
constexpr f4 F = {2.0f, __builtin_nanf(""), -0.0f, 0.5f};
constexpr u4 U = {0u, 4000000000u, 1u, 0u};
constexpr b4 B = {true, false, false, true};
constexpr h4 H = {(_Float16)1.0f, (_Float16)0.0f, (_Float16)-2.0f, (_Float16)0.0f};
struct S { static constexpr i4 M = {0, 7, 0, 7}; };

// Constant-initialized globals (no `__cxx_global_var_init`).
b4 g1 = __builtin_convertvector(K, b4);
i4 g2 = __builtin_convertvector(B, i4);
f4 g3 = __builtin_convertvector(B, f4);

template <class T, class V> V conv(V* out) {
  constexpr T k = {1, 0, 0, 1};
  *out = __builtin_convertvector(k, V);
  return __builtin_convertvector(S::M, V);
}

void f(b4* p, l4* q, d4* r, uc4* s, h4* t, i4* u) {
  const i4 cl = {0, 1, 0, 1};
  b4 a = __builtin_convertvector(K, b4);
  b4 b = __builtin_convertvector(F, b4);
  b4 c = __builtin_convertvector(U, b4);
  b4 d = __builtin_convertvector(H, b4);
  b4 e = __builtin_convertvector(cl, b4);
  b4 j = __builtin_convertvector(B, b4);
  l4 g = __builtin_convertvector(K, l4);
  l4 h = __builtin_convertvector(B, l4);
  d4 i = __builtin_convertvector(B, d4);
  uc4 k = __builtin_convertvector(B, uc4);
  h4 m = __builtin_convertvector(B, h4);
  i4 n = __builtin_convertvector(F, i4);
  b4 o = conv<i4, b4>(p);
  *p = a & b & c & d & e & j & o;
  *q = g + h;
  *r = i;
  *s = k;
  *t = m;
  *u = n;
}
