// [over.match.class.deduct]/3: the guide formed for an alias template
// from a guide f has f's associated constraints with the deduced
// arguments substituted, conjoined with the deducibility constraint.
// A guide declared in a class template keeps the constraint naming the
// enclosing class's parameter (`requires C<Z>`), and a fold over a pack
// the alias leaves undeduced stays a fold over the new guide's pack.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 44:6 error "no viable constructor or deduction guide for deduction of template arguments of 'LFoo'"
//@ diag: 42:20 note "candidate template ignored: constraints not satisfied [with V = int]"
//@ diag: 33:31 note "because 'C<long>' evaluated to false"
//@ diag: 42:20 note "implicit deduction guide declared as 'template <class V> requires C<long> && __is_deducible(LFoo, Foo<int>) LFoo(V *, V *) -> Foo<int>'"
//@ diag: 42:20 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 42:20 note "implicit deduction guide declared as 'template <class K> requires __is_deducible(LFoo, Outer<long>::Foo<K>) LFoo(K) -> Outer<long>::Foo<K>'"
//@ diag: 42:20 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 42:20 note "implicit deduction guide declared as 'template <class K> requires __is_deducible(LFoo, Outer<long>::Foo<K>) LFoo(Outer<long>::Foo<K>) -> Outer<long>::Foo<K>'"
//@ diag: 42:20 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 42:20 note "implicit deduction guide declared as 'template <class V> requires C<long> && __is_deducible(LFoo, Foo<int>) LFoo(V) -> Foo<int>'"
//@ diag: 55:3 error "no viable constructor or deduction guide for deduction of template arguments of 'B'"
//@ diag: 54:24 note "candidate template ignored: constraints not satisfied [with Us = <>, Ts = <double>]"
//@ diag: 49:43 note "because 'same<T2<>, double>' evaluated to false"
//@ diag: 46:44 note "because '__is_same(T2<>, double)' evaluated to false"
//@ diag: 54:24 note "implicit deduction guide declared as 'template <class ...Us, class ...Ts> requires (... && same<T2<Us...>, Ts>) && __is_deducible(B, T2<T2<Us...>>) B(Ts ...) -> T2<T2<Us...>>'"
//@ diag: 54:24 note "candidate template ignored: could not match 'T2<T2<Us...>>' against 'double'"
//@ diag: 54:24 note "implicit deduction guide declared as 'template <class ...Us> requires __is_deducible(B, T2<T2<Us...>>) B(T2<T2<Us...>>) -> T2<T2<Us...>>'"
//@ diag: 54:24 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 54:24 note "implicit deduction guide declared as 'template <class ...Us> requires __is_deducible(B, T2<T2<Us...>>) B() -> T2<T2<Us...>>'"
template <class T> constexpr bool C = __is_same(T, int);
template <class Z> struct Outer {
  template <class U> struct Foo { Foo(U); };
  template <class V> requires C<Z> Foo(V *, V *) -> Foo<int>;
  template <class V> requires C<Z> Foo(V) -> Foo<int>;
};
template <class K> using IFoo = Outer<int>::Foo<K>;
IFoo a(1.0); // the user guide: its constraint holds
static_assert(__is_same(decltype(a), Outer<int>::Foo<int>));
template <class Y> struct N { template <class K> using AFoo = Outer<Y>::template Foo<K>; };
N<long>::AFoo b(1.0); // the user guide's constraint fails: the constructor's
static_assert(__is_same(decltype(b), Outer<long>::Foo<double>));
template <class K> using LFoo = Outer<long>::Foo<K>;
int *p;
LFoo x(p, p);

template <class T, class U> concept same = __is_same(T, U);
template <class... E> struct T2 {
  T2();
  template <class... Ts> requires (... && same<E, Ts>) T2(Ts...);
};
template <class... Us> using A = T2<T2<Us...>>;
using X = decltype(A(T2<>()));
using X = T2<T2<>>;
template <class... Us> using B = T2<T2<Us...>>;
B y(1.0);
