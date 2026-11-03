// [stmt.return]/6, [diff.cpp23.stmt]: before C++26 (P2748R5) a returned
// reference bound to a temporary is well-formed but dangles; clang warns
// (-Wreturn-stack-address) where C++26 has the error, inside an instantiation
// too. The C++26 sibling is returned-temporary-make-from-tuple.cpp. Verified
// against clang 19 and 22.
//@ std: c++23
//@ strict-diags
//@ diag: 31:12 warning "returning reference to local temporary object"
//@ diag: 35:10 note "in instantiation of function template specialization 'make_from_tuple_impl<const int &, tuple<char>, 0>' requested here"
//@ diag: 37:27 note "in instantiation of function template specialization 'make_from_tuple<const int &, tuple<char>>' requested here"
//@ diag: 41:34 warning "returning reference to local temporary object"
//@ diag: 42:43 warning "returning reference to local temporary object"
//@ diag: 43:55 warning "returning reference to local temporary object"
//@ diag: 44:31 warning "returning reference to local temporary object"
//@ diag: 45:46 warning "reference to stack memory associated with parameter 'c' returned"
//@ diag: 46:63 warning "returning reference to local temporary object"
//@ diag: 46:48 note "binding reference variable 'r2' here"
//@ diag: 46:30 note "binding reference variable 'r' here"
template <class T> struct remove_ref { using type = T; };
template <class T> struct remove_ref<T&> { using type = T; };
template <class T> struct remove_ref<T&&> { using type = T; };
template <class T> constexpr T&& fwd(typename remove_ref<T>::type& t) noexcept {
  return static_cast<T&&>(t);
}
template <int... I> struct index_seq {};
template <class... Ts> struct tuple;
template <class A> struct tuple<A> { A a; };
template <int I, class A> constexpr A&& get(tuple<A>&& t) { return static_cast<A&&>(t.a); }
template <class T, class Tuple, int... I>
constexpr T make_from_tuple_impl(Tuple&& t, index_seq<I...>) {
  return T(get<I>(fwd<Tuple>(t))...);
}
template <class T, class Tuple>
constexpr T make_from_tuple(Tuple&& t) {
  return make_from_tuple_impl<T>(fwd<Tuple>(t), index_seq<0>{});
}
const int& bad() { return make_from_tuple<const int&>(tuple<char>{'a'}); }
const int& fine(tuple<int>&& t) { return make_from_tuple<const int&>(static_cast<tuple<int>&&>(t)); }

using CR = const int&;
const int& b(char c) { return CR(c); }
const int& c(char c) { return (const int&)c; }
const int& d(char c) { return static_cast<const int&>(c); }
const int& e(long c) { return c; }
const int& param(int c) { return (const int&)c; }
const int& f2() { const int& r = 3; const int& r2 = r; return r2; }
