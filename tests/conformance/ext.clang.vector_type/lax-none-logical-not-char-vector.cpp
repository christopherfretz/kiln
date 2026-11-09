// GNU vector `!` ([expr.unary.op] extended by clang's vector semantics):
// the result of `!v` on a vector of 8-bit integers is a vector of plain
// `char` (clang `GetSignedVectorType`) whatever the element signedness or
// the target's `char` signedness, and under `-flax-vector-conversions=none`
// it does not convert to a vector of `unsigned char` or `signed char`.
// libc++ 22 `experimental/__simd/vec_ext.h` `__negate` returns `{!__s.__data}`
// into an `unsigned char` mask; `simd_unary.pass.cpp` therefore adds
// `ADDITIONAL_COMPILE_FLAGS(clang): -flax-vector-conversions=integer`,
// which the libc++ runner applies to kiln (WP-980; the sibling
// lax-integer-logical-not-char-vector.cpp accepts the same code).
//@ expect: compile-fail
//@ args: -flax-vector-conversions=none
//@ strict-diags
//@ diag: 26:20 error "cannot initialize a member subobject of type 'U' (vector of 16 'unsigned char' values) with an rvalue of type '__attribute__((__vector_size__(16 * sizeof(char)))) char' (vector of 16 'char' values)"
//@ diag: 29:29 error "cannot initialize a member subobject of type 'U' (vector of 16 'unsigned char' values) with an rvalue of type '__attribute__((__vector_size__(16 * sizeof(char)))) char' (vector of 16 'char' values)"
//@ diag: 31:32 note "in instantiation of member function 'N<int>::g' requested here"
//@ diag: 32:21 error "cannot initialize return object of type 'S' (vector of 16 'signed char' values) with an rvalue of type '__attribute__((__vector_size__(16 * sizeof(char)))) char' (vector of 16 'char' values)"
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
