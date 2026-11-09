// GNU vector `!` under `-flax-vector-conversions=integer` (clang's default
// is `integer` too): the plain-`char` vector result of `!` converts to any
// 16-byte integer vector, as libc++ 22 `simd_unary.pass.cpp` needs
// (`ADDITIONAL_COMPILE_FLAGS(clang)`; WP-980; the sibling
// lax-none-logical-not-char-vector.cpp rejects it under `none`).
//@ expect: compile-ok
//@ args: -flax-vector-conversions=integer
typedef unsigned char U __attribute__((vector_size(16)));
typedef signed char S __attribute__((vector_size(16)));
typedef char C __attribute__((vector_size(16)));
struct M {
  U d;
};
C plain(U u) { return !u; }
C plain(S s) { return !s; }
M f(U u) { return {!u}; }
template <class T> struct N {
  U d;
  static N g(U u) { return {!u}; }
};
N<int> h(U u) { return N<int>::g(u); }
S sgn(U u) { return !u; }
