// [dcl.struct.bind]/4: the binding's type is tuple_element<i, E>::type with
// i a std::size_t value converted to the template parameter's type, so a
// user `tuple_element` declared with an `unsigned` index still matches its
// explicit specialization `tuple_element<0, T>`. Under -fclang-compat=19 kiln
// passes the index unconverted and misses the specialization (rejects-valid).
//@ requires: clang<22
//@ expect: compile-ok
//@ xfail(kiln): WP-891 follow-up, compat-19 tuple_element index is a size_t value not converted to the parameter type (struct_bind.rs tuple_element_type)

namespace std {
template <class T> struct tuple_size;
template <unsigned I, class T> struct tuple_element;
} // namespace std
struct P { int a; template <unsigned I> int get() const { return a; } };
template <> struct std::tuple_size<P> { static constexpr unsigned value = 1; };
template <> struct std::tuple_element<0, P> { using type = int; };
int f() {
  auto [x] = P{1};
  return x;
}
