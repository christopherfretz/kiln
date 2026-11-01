// [over.match.class.deduct]/1.5-1.8 (P1816): aggregate deduction
// candidates -- braced, parenthesized and designated initializers,
// brace elision into arrays and non-dependent sub-aggregates,
// `T(&&)[N]` / `const T(&)[N]` array elements, and a trailing pack base.
//@ strict-diags
template<class T, class U> struct Agg { T a; U b; };
Agg g{1, 2.0};
static_assert(__is_same(decltype(g), Agg<int, double>));
Agg g2(1, 'c');
static_assert(__is_same(decltype(g2), Agg<int, char>));
Agg g3{.a = 1L, .b = 2u};
static_assert(__is_same(decltype(g3), Agg<long, unsigned>));
template<class T> struct Arr { T a[3]; };
Arr ar{1, 2, 3};
static_assert(__is_same(decltype(ar), Arr<int>));
Arr ar2{{1.0, 2.0, 3.0}};
static_assert(__is_same(decltype(ar2), Arr<double>));
template<class T, int N> struct Buf { T s[N]; };
Buf b{"abc"};
static_assert(__is_same(decltype(b), Buf<char, 4>));
Buf b2{{1, 2}};
static_assert(__is_same(decltype(b2), Buf<int, 2>));
struct In { int x, y; };
template<class T> struct Nest { In i; T t; };
Nest n{1, 2, 3.0};
static_assert(__is_same(decltype(n), Nest<double>));
template<class... Ts> struct overloaded : Ts... { };
struct F1 { int x; }; struct F2 { };
overloaded o{F1{1}, F2{}};
static_assert(__is_same(decltype(o), overloaded<F1, F2>));
overloaded o0{};
static_assert(__is_same(decltype(o0), overloaded<>));
template<class B, class T> struct D : B { T t; };
struct Base { int k; };
D d{Base{1}, 2.0};
static_assert(__is_same(decltype(d), D<Base, double>));
