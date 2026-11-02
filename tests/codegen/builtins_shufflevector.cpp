// GNU __builtin_shufflevector (clang EmitBuiltinExpr ->
// ShuffleVectorExpr): one `shufflevector` instruction with a constant
// mask; index -1 becomes a poison lane, and the result has as many lanes
// as there are indices (a generic vector of the element type). The unary
// form `(v, mask)` masks the lanes to the next power of two of the length
// and builds the result one extractelement/insertelement pair per lane.
typedef int v4 __attribute__((ext_vector_type(4)));
typedef float f4 __attribute__((vector_size(16)));
v4 s1(v4 a, v4 b) { return __builtin_shufflevector(a, b, 0, 5, -1, 3); }
auto s2(v4 a) { return __builtin_shufflevector(a, a, 3, 2); }
f4 s3(f4 a, f4 b) { return __builtin_shufflevector(a, b, 7, 6, 5, 4); }
template <int I> v4 s4(v4 a) { return __builtin_shufflevector(a, a, I, I + 1, 0, 0); }
v4 s5(v4 a) { return s4<2>(a); }
auto s6(v4 a) { return __builtin_shufflevector(a + 1, a, 0, 1, 2, 3, 4, 5, 6, 7); }
typedef unsigned u4 __attribute__((vector_size(16)));
typedef int i4 __attribute__((vector_size(16)));
typedef short sh3 __attribute__((ext_vector_type(3)));
typedef char ch3 __attribute__((ext_vector_type(3)));
i4 u1(i4 x, u4 m) { return __builtin_shufflevector(x, m); }
sh3 u2(sh3 x, ch3 m) { return __builtin_shufflevector(x, m); }
f4 u3(f4 x) {
  i4 m = {3, 2, 1, 0};
  return __builtin_shufflevector(x, m);
}
