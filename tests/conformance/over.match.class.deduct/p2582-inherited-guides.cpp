// [over.match.class.deduct]/1 (C++23, P2582R1): a class template that
// inherits constructors from a direct base B (a simple-template-id) gets the
// guides of the alias template `template<C's parameters> using A = B;` with
// each return type R replaced by the specialization of C whose arguments are
// deduced from R against B ([over.match.class.deduct]/1, example 2 there).
// [over.match.best.general]/2.10: a guide not formed from an inherited
// constructor beats one that is when their parameter types are the same.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ skip(clang): neither clang 19 nor clang 22 implements P2582; the standard's example is the contract
template <class X, class Y> constexpr bool same = false;
template <class X> constexpr bool same<X, X> = true;

// The standard's example (C and the user guide of B).
template <typename T> struct B { B(T); };
template <typename T> struct C : public B<T> { using B<T>::B; };
C c(42);
static_assert(same<decltype(c), C<int>>);
B(int) -> B<char>;
C c2(42);
static_assert(same<decltype(c2), C<char>>);
C c3(1.5);
static_assert(same<decltype(c3), C<double>>);

// The standard's G, with the base named by its template-id: the arguments of
// G are deduced from F<U, T, int>.
template <typename T, typename U, typename V> struct F { F(T, U, V); };
template <typename T, typename U> struct G : F<U, T, int> { using F<U, T, int>::F; };
G g(true, 'a', 1);
static_assert(same<decltype(g), G<char, bool>>);

// C's own copy deduction candidate.
C c5(c);
static_assert(same<decltype(c5), C<int>>);

// A constructor template of the base.
template <class T> struct H { template <class U> H(T, U*); };
template <class T> struct I : H<T> { using H<T>::H; };
I i(1L, (double*)nullptr);
static_assert(same<decltype(i), I<long>>);

// A user-declared guide template of the base.
template <class T> struct J { template <class U> J(U, T); };
template <class U> J(U, int) -> J<long>;
template <class T> struct K : J<T> { using J<T>::J; };
K k(1, 2);
static_assert(same<decltype(k), K<long>>);

// /2.10: M's own constructor's guide over the inherited one with the same
// parameter types (otherwise ambiguous).
template <class T> struct L { L(T); };
template <class T> struct M : L<T> { using L<T>::L; M(T); };
M m(3u);
static_assert(same<decltype(m), M<unsigned>>);

// Packs, and a base whose arguments repeat a parameter or wrap it.
template <class... Ts> struct P { P(Ts...); };
template <class... Ts> struct Q : P<Ts...> { using P<Ts...>::P; };
Q q(1, 2.0, 'c');
static_assert(same<decltype(q), Q<int, double, char>>);
template <class A1, class A2> struct R2 { R2(A1, A2); };
template <class X> struct S : R2<X, X> { using R2<X, X>::R2; };
S s(1, 2);
static_assert(same<decltype(s), S<int>>);
template <class X> struct V : B<X*> { using B<X*>::B; };
V v((int*)nullptr);
static_assert(same<decltype(v), V<int>>);

// In a function-style cast and a new-expression.
static_assert(same<decltype(C(1.0f)), C<float>>);
auto* pc = new C(2L);
static_assert(same<decltype(pc), C<long>*>);
