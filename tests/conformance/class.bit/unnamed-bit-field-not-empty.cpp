// [class.bit]/2, [meta.unary.prop] is_empty: an unnamed bit-field of
// non-zero width is storage, so its class is not empty (Itanium C++ ABI
// §1.1 "empty class"; clang/GCC agree); only a zero-width one keeps the
// class empty. The answer feeds the calling convention (spec/abi.md §8.6).
//@ expect: compile-pass
//@ strict-diags
struct U4 { int : 4; };
struct U0 { int : 0; };
struct U04 { int : 0; char : 1; };
struct D4 : U4 {};
struct D0 : U0 {};
struct N4 { [[no_unique_address]] U4 u; };
struct N0 { [[no_unique_address]] U0 u; };
static_assert(!__is_empty(U4));
static_assert(__is_empty(U0));
static_assert(!__is_empty(U04));
static_assert(!__is_empty(D4));
static_assert(__is_empty(D0));
static_assert(!__is_empty(N4));
static_assert(__is_empty(N0));
template <int W> struct T { int : W; };
static_assert(!__is_empty(T<3>));
static_assert(__is_empty(T<0>));
int main() {}
