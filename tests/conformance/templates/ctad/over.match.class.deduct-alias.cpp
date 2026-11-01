// [over.match.class.deduct]/3 (P1814): deduction through an alias template.
// The guides of the template the alias names, transformed: the return type
// deduced from the alias's right-hand side.
//@ strict-diags
template<class T, class U> struct P { P(T, U); };
template<class T> using Same = P<T, T>;
Same s(1, 2);
static_assert(__is_same(decltype(s), P<int, int>));
template<class T> using IntP = P<int, T>;
IntP ip(1L, 2.0);
static_assert(__is_same(decltype(ip), P<int, double>));
template<class T> using Ptr = P<T *, T>;
Ptr pp((int *)0, 1);
static_assert(__is_same(decltype(pp), P<int *, int>));

// An alias of an alias; functional cast and new-expression forms.
template<class T> using Same2 = Same<T>;
Same2 s2(1, 2);
static_assert(__is_same(decltype(s2), P<int, int>));
auto fc = Same(1L, 2L);
static_assert(__is_same(decltype(fc), P<long, long>));
auto *np = new Same(1u, 2u);
static_assert(__is_same(decltype(np), P<unsigned, unsigned> *));

// Default template arguments and qualified names.
template<class T, class U = T> using Def = P<T, U>;
Def df(1, 'c');
static_assert(__is_same(decltype(df), P<int, char>));
namespace N { template<class T> using NS = P<T, char>; }
N::NS ns(1.0f, 'c');
static_assert(__is_same(decltype(ns), P<float, char>));

// Constructor templates, the copy deduction candidate, other init forms.
template<class T> struct V { V(T); template<class It> V(It, It); };
template<class T> using W = V<T>;
W w(1);
static_assert(__is_same(decltype(w), V<int>));
W wc = w;
static_assert(__is_same(decltype(wc), V<int>));
template<class T> struct X { template<class U> X(T, U); };
template<class T> using XA = X<T>;
XA xa(1, 2.0);
static_assert(__is_same(decltype(xa), X<int>));
template<class T> struct C { C(T); C(T, int); };
template<class T> using CC = C<T>;
CC cc{1};
static_assert(__is_same(decltype(cc), C<int>));
const CC cc2 = 2.0;
static_assert(__is_same(decltype(cc2), const C<double>));

// User-declared guides and the aggregate deduction candidate.
template<class T> struct G { G(T); };
G(const char *) -> G<long>;
template<class T> G(T *) -> G<T>;
template<class T> using GA = G<T>;
GA ga(1.0);
static_assert(__is_same(decltype(ga), G<double>));
template<class T> struct Ag { T a; T b; };
template<class T> using AgA = Ag<T>;
AgA aa{1, 2};
static_assert(__is_same(decltype(aa), Ag<int>));

// Packs and non-type parameters.
template<class... Ts> struct Tup { Tup(Ts...); };
template<class... Us> using TA = Tup<Us...>;
TA t(1, 2.0, 'c');
static_assert(__is_same(decltype(t), Tup<int, double, char>));
template<class... Us> using TI = Tup<int, Us...>;
TI ti(1, 2.0);
static_assert(__is_same(decltype(ti), Tup<int, double>));
template<class T, int N> struct Arr { Arr(const T (&)[N]); };
template<int N> using CA = Arr<char, N>;
CA ca("abc");
static_assert(__is_same(decltype(ca), Arr<char, 4>));
