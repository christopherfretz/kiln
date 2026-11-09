// [dcl.struct.bind]/4, [temp.arg.nontype]/2: the index of
// tuple_element<i, E> is converted to the template parameter's type, an
// `unsigned char` here; clang 22 prints the std::size_t argument as
// written (0UL) in the binding's type (WP-914; the -19 twin prints the
// converted one). The binding's reference type `int *`
// cannot bind to the `int` that `get` returns.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:9 error "reference to type 'std::tuple_element<0UL, P>::type' (aka 'int *') could not bind to an rvalue of type 'int'"
//@ diag: 20:9 note "in implicit initialization of binding declaration 'x'"
namespace std {
template <class T> struct tuple_size;
template <unsigned char I, class T> struct tuple_element;
} // namespace std
struct P { int a; template <unsigned I> int get() const { return a; } };
template <> struct std::tuple_size<P> { static constexpr unsigned value = 1; };
template <> struct std::tuple_element<0, P> { using type = int *; };
int f() {
  auto [x] = P{1};
  return x;
}
