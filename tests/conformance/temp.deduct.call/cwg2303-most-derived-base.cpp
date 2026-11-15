// [temp.deduct.call]/4.3 (CWG2303): when P is a class template-id and
// several base classes of the argument's class match, a match B is no
// candidate when another match is derived from it, so the most derived one
// is deduced; the argument type itself matching needs no base walk; access
// and ambiguity are not part of deduction. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -Wno-inaccessible-base
template <class... T> struct A {};
template <> struct A<> {};
template <class H, class... R> struct A<H, R...> : A<R...> {};
template <class... T> constexpr int f(const A<T...> &) { return sizeof...(T); }
template <class... T> constexpr int fp(const A<T...> *) { return sizeof...(T); }

struct B : A<int, int> {};
struct C : A<int, int>, A<int> {};
struct D : A<int>, A<int, int> {};
static_assert(f(A<int, char, long>{}) == 3);
static_assert(f(B{}) == 2 && f(C{}) == 2 && f(D{}) == 2);

// The pointer form.
constexpr B b{};
constexpr C c{};
static_assert(fp(&b) == 2 && fp(&c) == 2);

// A match below another one reached through a second path.
struct M2 : A<int> {};
struct M : A<int, int>, M2 {};
static_assert(f(M{}) == 2);

// A virtual base reached twice is one candidate.
struct V1 : virtual A<char> {};
struct V2 : virtual A<char> {};
struct VD : V1, V2 {};
int vd(const VD &x) { return f(x); }

// A private base deduces; the conversion is checked where it is accessible.
struct P : private A<int, short> {
  friend int pf();
};
int pf() { return f(P{}); }

// With a pack, a non-deduced context and a partly explicit argument list.
template <class T> struct id { using type = T; };
template <class T, class... U>
constexpr int g(const A<T, U...> &, typename id<T>::type) { return 1 + sizeof...(U); }
static_assert(g(B{}, 0) == 2);
static_assert(g<int>(C{}, 0) == 2);
static_assert(g<int, int>(D{}, 0) == 2);

// Partial ordering between a direct match and one through the bases.
template <class T> constexpr int h(const A<T> &) { return 1; }
template <class... T> constexpr int h(const A<T...> &) { return 2; }
static_assert(h(B{}) == 2);
struct E1 : A<int> {};
static_assert(h(E1{}) == 1);

// Matching a partial specialization never looks at bases.
template <class T> struct S { static constexpr int v = 0; };
template <class... T> struct S<A<T...>> { static constexpr int v = 1; };
static_assert(S<A<int>>::v == 1 && S<B>::v == 0);

// The same inside a template.
template <class T> constexpr bool in_tmpl(T t) { return f(t) == 2 && fp(&t) == 2; }
static_assert(in_tmpl(C{}) && in_tmpl(D{}));
