// [temp.deduct.general]/5, /8, [temp.inst]/1: an error inside a member
// typedef of a class template specialization instantiated while the
// deduced arguments are substituted into a function type is not in the
// immediate context: it is a hard error (`plain`). From clang 22 the
// associated constraints are checked before that substitution (CWG2369),
// so the `get_default` candidate whose requires-clause rejects it never
// instantiates `Nested<const Map *, 1>` and the other overload is chosen
// (the shape of a nested-map lookup helper, folly
// `folly/container/MapUtil.h:337`; WP-761). member-typedef-hard-error-19.cpp
// has clang 19, which substitutes first.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -ferror-limit=0
//@ diag: 38:57 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 39:46 note "in instantiation of template class 'Plain<int>' requested here"
//@ diag: 41:15 note "while substituting deduced template arguments into function template 'plain' [with T = int]"
template <class T> struct remove_pointer { using type = T; };
template <class T> struct remove_pointer<T*> { using type = T; };
template <class T, int L> requires (L > 0)
struct Nested {
  using type = typename Nested<typename remove_pointer<T>::type, L - 1>::type::mapped_type;
};
template <class T> struct Nested<T, 1> { using type = typename T::mapped_type; };

struct Map { using key_type = int; using mapped_type = int; };

template <class M, class K = typename M::key_type>
typename M::mapped_type get_default(const M* m, const K& k, int dflt);

template <class M, class K1, class K2, class... Ks> requires (sizeof...(Ks) != 0)
auto get_default(const M& m, const K1&, const K2&, const Ks&...)
    -> typename Nested<M, 1 + sizeof...(Ks)>::type;

int use(const Map* p) { return get_default(p, 1, 2); }

template <class T> struct Plain { using type = typename T::mapped_type; };
template <class T> auto plain(T) -> typename Plain<T>::type;
void plain(...);
void use2() { plain(0); }
