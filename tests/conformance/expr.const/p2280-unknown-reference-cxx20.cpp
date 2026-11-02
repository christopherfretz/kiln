// [expr.const]/5.12 (C++20, before P2280R4): an id-expression naming a
// reference must refer to a variable usable in constant expressions or
// whose lifetime began within the evaluation. A reference parameter
// outside its call has an unknown value, so even naming it is not
// constant (clang applies P2280 from C++23 only).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 20:47 error "static assertion expression is not an integral constant expression"
//@ diag: 20:52 note "function parameter 'a' with unknown value cannot be used in a constant expression"
//@ diag: 20:24 note "declared here"
//@ diag: 22:46 error "static assertion expression is not an integral constant expression"
//@ diag: 22:46 note "function parameter 's' with unknown value cannot be used in a constant expression"
//@ diag: 22:27 note "declared here"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:16 note "initializer of 'er' is unknown"
//@ diag: 23:13 note "declared here"
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
void array_param(int (&a)[4]) { static_assert(size(a) == 4); }
struct S { constexpr int three() const { return 3; } };
void class_param(const S &s) { static_assert(s.three() == 3); }
extern int &er;
static_assert(&er == &er);
