// [stmt.return]/6 (P2748R5), [tuple.apply]: std::make_from_tuple's shape.
// Constructing a reference result from a tuple element of another type
// binds it to a temporary; libc++ rejects that with a static_assert, and the
// return itself is ill-formed in C++26 (clang: an error, also inside the
// instantiation). The -cxx23 sibling has the pre-C++26 warning. Verified
// against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:17 error "static assertion failed due to requirement '!__reference_constructs_from_temporary(const int &, char &&)': binds to a temporary whose lifetime has ended"
//@ diag: 36:10 note "in instantiation of function template specialization 'make_from_tuple_impl<const int &, tuple<char>, 0>' requested here"
//@ diag: 38:27 note "in instantiation of function template specialization 'make_from_tuple<const int &, tuple<char>>' requested here"
//@ diag: 32:12 error "returning reference to local temporary object"
//@ diag: 44:34 error "returning reference to local temporary object"
//@ diag: 45:43 error "returning reference to local temporary object"
//@ diag: 46:55 error "returning reference to local temporary object"
//@ diag: 47:31 error "returning reference to local temporary object"
//@ diag: 48:46 warning "reference to stack memory associated with parameter 'c' returned"
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
  static_assert(!__reference_constructs_from_temporary(T, decltype(get<I>(fwd<Tuple>(t)))...),
                "binds to a temporary whose lifetime has ended");
  return T(get<I>(fwd<Tuple>(t))...);
}
template <class T, class Tuple>
constexpr T make_from_tuple(Tuple&& t) {
  return make_from_tuple_impl<T>(fwd<Tuple>(t), index_seq<0>{});
}
const int& bad() { return make_from_tuple<const int&>(tuple<char>{'a'}); }
const int& fine(tuple<int>&& t) { return make_from_tuple<const int&>(static_cast<tuple<int>&&>(t)); }

// The same binding without the template: a functional cast, a C-style cast,
// a static_cast and an implicit conversion; a parameter is the other warning.
using CR = const int&;
const int& b(char c) { return CR(c); }
const int& c(char c) { return (const int&)c; }
const int& d(char c) { return static_cast<const int&>(c); }
const int& e(long c) { return c; }
const int& param(int c) { return (const int&)c; }
