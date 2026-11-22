//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 26:32 error "non-type template argument is not a constant expression"
//@ diag: 26:32 note "reference to 'b' is not a constant expression"
//@ diag: 26:20 note "declared here"
//@ diag: 27:32 error "non-type template argument is not a constant expression"
//@ diag: 27:32 note "pointer to 'a' is not a constant expression"
//@ diag: 27:17 note "declared here"
// [temp.arg.nontype]/2, [expr.const]/14, [dcl.struct.bind]/7 (CWG 3135):
// the variable a tuple-like binding names when get<i> returns a prvalue
// has the storage duration of the structured binding declaration, so an
// automatic one is no constant template argument (clang 22 agrees, from
// its reference model: the temporary is automatic). The static ones are
// valid (cwg3135-tuple-binding-arg-run.cpp).
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
} // namespace std
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 1; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
template <const int& R> struct S {};
template <const int* P> struct T {};
int f() { auto [a, b] = X{}; S<b> s; return 0; }
int g() { auto [a, b] = X{}; T<&a> t; return 0; }
