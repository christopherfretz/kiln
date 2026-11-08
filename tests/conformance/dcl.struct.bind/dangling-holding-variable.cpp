// [dcl.struct.bind]/4, [class.temporary]/6 (WP-875): the hidden variable
// that holds each tuple-like binding's get<I>() result is checked like a
// named reference; a temporary it reaches through a lifetimebound parameter
// dangles, and clang warns with a note naming the binding.
//@ strict-diags
//@ expect: compile-ok
//@ diag: 24:9 warning "temporary implicitly bound to local reference will be destroyed at the end of the full-expression"
//@ diag: 24:9 note "in implicit initialization of binding declaration 'a'"

namespace std {
template <class T> struct tuple_size;
template <decltype(sizeof 0) I, class T> struct tuple_element;
} // namespace std
struct T6 { int a; };
struct Wrap { Wrap(const T6 &); int v; };
template <> struct std::tuple_size<T6> { static constexpr decltype(sizeof 0) value = 1; };
template <> struct std::tuple_element<0, T6> { using type = const int &; };
template <decltype(sizeof 0) I> const int &get(const Wrap &w [[clang::lifetimebound]]);
struct T7 { int a; };
template <> struct std::tuple_size<T7> { static constexpr decltype(sizeof 0) value = 1; };
template <> struct std::tuple_element<0, T7> { using type = const int &; };
template <decltype(sizeof 0) I> const int &get(const T7 &t [[clang::lifetimebound]]);
void h() {
  auto [a] = T6{1};
  auto [b] = T7{2};
  T7 t{3};
  auto &[c] = t;
  (void)a; (void)b; (void)c;
}
