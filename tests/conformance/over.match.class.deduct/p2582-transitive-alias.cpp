// [over.match.class.deduct]/1 (C++23, P2582R1): the guides C inherits from
// its base B are "the guides of A", the alias template formed from B; when
// B itself inherits constructors those guides include B's inherited ones
// (transitively, D : B<T*> : A<T*>), and CTAD through an alias template of
// an inheriting class (/3, P1814) sees them as well. A pack expansion in the
// base deduces the whole pack (Q2 : Q<Ts...> : P<Ts...>). `using G::F::F`
// (the standard's example, base named through G's injected-class-name).
// The return type printed for an inherited guide is the deriving class's
// specialization (CC<R>::type, D<T>), not the base's.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ skip(clang): neither clang 19 nor clang 22 implements P2582; the standard's example is the contract
//@ diag: 55:3 error "no viable constructor or deduction guide for deduction of template arguments of 'D'"
//@ diag: 34:27 note "candidate template ignored: could not match 'D<T>' against 'int'"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> D(D<T>) -> D<T>'"
//@ diag: 34:27 note "candidate template ignored: could not match 'B<T *>' against 'int'"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(D, B<T *>) D(B<T *>) -> D<T>'"
//@ diag: 34:27 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(D, B<T *>) D(T *) -> D<T>'"
//@ diag: 34:27 note "candidate template ignored: could not match 'A<T *>' against 'int'"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(D, B<T *>) D(A<T *>) -> D<T>'"
//@ diag: 34:27 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> D() -> D<T>'"
//@ diag: 34:27 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 34:27 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(D, B<T *>) D() -> D<T>'"
template <class X, class Y> constexpr bool same = false;
template <class X> constexpr bool same<X, X> = true;
template <class T> struct A { A(T); };
template <class T> struct B : A<T> { using A<T>::A; };
template <class T> struct C : B<T> { using B<T>::B; };
C c(1);
static_assert(same<decltype(c), C<int>>);
template <class T> struct D : B<T*> { using B<T*>::B; };
D d((int*)0);
static_assert(same<decltype(d), D<int>>);
template <class T> using AB = B<T>;
AB ab(2.0);
static_assert(same<decltype(ab), B<double>>);
template <class T> using AC = C<T>;
AC ac('c');
static_assert(same<decltype(ac), C<char>>);
template <class... Ts> struct P { P(Ts...); };
template <class... Ts> struct Q : P<Ts...> { using P<Ts...>::P; };
template <class... Ts> struct Q2 : Q<Ts...> { using Q<Ts...>::Q; };
Q2 q2(1, 2.0);
static_assert(same<decltype(q2), Q2<int, double>>);
template <class... Ts> using AQ = Q<Ts...>;
AQ aq(1, 'c');
static_assert(same<decltype(aq), Q<int, char>>);
template <typename T, typename U, typename V> struct F { F(T, U, V); };
template <typename T, typename U> struct G : F<U, T, int> { using G::F::F; };
G g(true, 'a', 1);
static_assert(same<decltype(g), G<char, bool>>);
D de(1);
