// [temp.param]/5, [temp.constr.decl]/3.1: a non-type template parameter
// declared `C auto V` has the immediately-declared constraint
// `C<decltype(V)>` (a fold over the pack for `C auto... Vs`) among its
// associated constraints, so it decides which partial specialization
// matches ([temp.spec.partial.match]) and which is more specialized
// ([temp.func.order], subsumption), and an argument that does not satisfy
// it is rejected.
//@ verify
template <class T> concept A = requires { typename T::a; };
template <class T> concept B = !A<T>;
struct E { using a = int; };
struct F {};

template <auto> struct q;
template <A auto D> struct q<D> { static constexpr int v = 1; };
template <B auto D> struct q<D> { static constexpr int v = 2; };
static_assert(q<E{}>::v == 1);
static_assert(q<F{}>::v == 2);

template <auto> struct r { static constexpr int v = 0; };
template <A auto D> struct r<D> { static constexpr int v = 1; };
static_assert(r<F{}>::v == 0);
static_assert(r<1>::v == 0);
static_assert(r<E{}>::v == 1);

template <class T, auto...> struct qs;
template <class S, A auto D, B auto... Args> struct qs<S, D, Args...> { static constexpr int v = 1; };
template <class S, B auto D, B auto... Args> struct qs<S, D, Args...> { static constexpr int v = 2; };
struct X1 final : qs<X1, E{}> {};
struct X2 final : qs<X2, F{}, 1, 2> {};
static_assert(X1::v == 1);
static_assert(X2::v == 2);

template <class T> concept Int = __is_same(T, int);
template <class T> concept SmallInt = Int<T> && sizeof(T) == 4;
template <auto> struct s { static constexpr int v = 0; };
template <Int auto D> struct s<D> { static constexpr int v = 1; };
template <SmallInt auto D> struct s<D> { static constexpr int v = 2; };
static_assert(s<1>::v == 2);
static_assert(s<1L>::v == 0);

template <Int auto D> constexpr int f() { return 1; }
template <SmallInt auto D> constexpr int f() { return 2; }
static_assert(f<1>() == 2);

template <Int auto... Ds> struct Y { static constexpr int n = sizeof...(Ds); };
static_assert(Y<1, 2, 3>::n == 3);

template <class T> concept Wide = sizeof(T) == 8; // expected-note {{because 'sizeof(int) == 8' (4 == 8) evaluated to false}}
template <Wide auto D> struct W {}; // expected-note {{because 'decltype(1)' (aka 'int') does not satisfy 'Wide'}}
W<1L> w1;
W<1> w2; // expected-error {{constraints not satisfied for class template 'W' [with D = 1]}}
