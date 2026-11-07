// [temp.deduct.general]/8, [temp.inst]/1: an error inside a member
// typedef of a class template specialization instantiated while the
// deduced arguments are substituted into a function type is not in the
// immediate context: it is a hard error, not a substitution failure. At
// clang 19 the associated constraints are checked only after that
// substitution (spec/sema-templates.md Q12, clang 19
// `FinishTemplateArgumentDeduction`), so the `get_default` candidate whose
// requires-clause would reject it still instantiates `Nested<const Map *,
// 1>` (the shape of a nested-map lookup helper, folly
// `folly/container/MapUtil.h:337`; WP-761). member-typedef-hard-error.cpp
// has clang 22, which checks the constraints first.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ args: -ferror-limit=0
//@ diag: 29:64 error "type 'const Map *' cannot be used prior to '::' because it has no members"
//@ diag: 38:17 note "in instantiation of template class 'Nested<const Map *, 1>' requested here"
//@ diag: 40:32 note "while substituting deduced template arguments into function template 'get_default' [with M = const Map *, K1 = int, K2 = int, Ks = <>]"
//@ diag: 42:57 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 43:46 note "in instantiation of template class 'Plain<int>' requested here"
//@ diag: 45:15 note "while substituting deduced template arguments into function template 'plain' [with T = int]"
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
