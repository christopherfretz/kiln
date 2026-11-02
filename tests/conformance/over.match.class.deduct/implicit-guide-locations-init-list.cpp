// [over.match.class.deduct]/1.2, [temp.deduct.call]/1: the copy and simple
// `C() -> C<T>` guides sit at the class template's definition, and an
// initializer-list element that cannot be deduced is reported against the
// element type as written (`pr<const K, T>`), both through a member typedef in
// a guide and directly in a function template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:4 error "no viable constructor or deduction guide for deduction of template arguments of 'mp'"
//@ diag: 32:76 note "candidate template ignored: could not match 'value_type' (aka 'pr<const K, T>') against 'int'"
//@ diag: 32:76 note "implicit deduction guide declared as 'template <class K, class T> mp(std::initializer_list<value_type>, int, int) -> mp<K, T>'"
//@ diag: 32:36 note "candidate function template not viable: requires 1 argument, but 3 were provided"
//@ diag: 32:36 note "implicit deduction guide declared as 'template <class K, class T> mp(mp<K, T>) -> mp<K, T>'"
//@ diag: 35:12 error "no matching function for call to 'f'"
//@ diag: 33:34 note "candidate template ignored: could not match 'pr<const K, T>' against 'int'"
//@ diag: 35:30 error "no viable constructor or deduction guide for deduction of template arguments of 's::arr'"
//@ diag: 29:34 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 29:34 note "implicit deduction guide declared as 'template <class T, int N> arr() -> arr<T, N>'"
//@ diag: 29:34 note "candidate function template not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:34 note "implicit deduction guide declared as 'template <class T, int N> arr(arr<T, N>) -> arr<T, N>'"
namespace std {
template <class E> class initializer_list {
  const E *b; decltype(sizeof 0) n;
public:
  constexpr initializer_list() noexcept : b(nullptr), n(0) {}
};
}
namespace s {
template <class T, int N> struct arr;
template <class T, int N> struct arr { T e[N]; };
}
template <class A, class B> struct pr { pr(A, B); };
template <class K, class T> struct mp { using value_type = pr<const K, T>; mp(std::initializer_list<value_type>, int, int); };
template <class K, class T> void f(std::initializer_list<pr<const K, T>>);
mp m({1, 2}, 1, 1);
void g() { f({1, 2}); s::arr a; }
