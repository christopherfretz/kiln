// __integer_pack's rejected forms: a use outside a template, one not
// expanded, a negative N, an N above kiln's limit for integer sequences
// (__make_integer_seq's), an N that is not an integral constant expression
// or not of integral or unscoped enumeration type, a length that differs
// from another pack's in the same expansion. GCC rejects the same lines
// with its own wording (its limit is 268435452). Found running GCC's C++
// testsuite. WP-1667.
//@ std: c++20
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __integer_pack
//@ strict-diags
//@ diag: 29:3 error "use of built-in parameter pack '__integer_pack' outside of a template"
//@ diag: 30:9 error "use of built-in parameter pack '__integer_pack' outside of a template"
//@ diag: 32:45 error "initializer contains unexpanded parameter pack '__integer_pack'"
//@ diag: 34:45 error "integer sequences must have non-negative sequence length"
//@ diag: 35:1 note "in instantiation of template type alias 'S' requested here"
//@ diag: 34:45 error "integer sequence length 1048577 exceeds the implementation limit of 1048576"
//@ diag: 36:1 note "in instantiation of template type alias 'S' requested here"
//@ diag: 39:47 error "expression is not an integral constant expression"
//@ diag: 40:1 note "in instantiation of template type alias 'V' requested here"
//@ diag: 39:47 note "read of non-const variable 'value' is not allowed in a constant expression"
//@ diag: 38:30 note "declared here"
//@ diag: 42:48 error "integral constant expression must have integral or unscoped enumeration type, not 'double'"
//@ diag: 44:48 error "integral constant expression must have integral or unscoped enumeration type, not 'scoped'"
//@ diag: 46:61 error "pack expansion contains parameter packs '__integer_pack' and 'M' that have different lengths (2 vs. 3)"
//@ diag: 47:1 note "in instantiation of template type alias 'P' requested here"
template <int...> struct A {};
A<__integer_pack(2)...> a;
int i = __integer_pack(2);

template <int N> int unexpanded() { int x = __integer_pack(N); return x; }

template <int N> using S = A<__integer_pack(N)...>;
S<-1> negative;
S<1048577> too_long;

struct variable { static int value; };
template <class T> using V = A<__integer_pack(T::value)...>;
V<variable> not_constant;

template <double D> using F = A<__integer_pack(D)...>;
enum class scoped { two = 2 };
template <scoped N> using G = A<__integer_pack(N)...>;

template <int N, int... M> using P = A<(__integer_pack(N) + M)...>;
P<2, 1, 2, 3> mismatch;
