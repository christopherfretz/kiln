// [temp.arg.nontype]/2, [expr.const]/12: the converted constant
// expressions a pointer parameter does admit: an identity or
// qualification conversion (`&s` for `S *` and `const S *`), a null
// pointer conversion from std::nullptr_t, an explicit cast to
// `const void *`, and a `const void *` object of static storage
// (WP-407; pointer-conversion-not-allowed.cpp has the rejected shapes).
//@ expect: run
struct S { int v; };
S s{1};
const S cs{2};
constexpr const void *cvp = &s;
template <const void *P> constexpr const void *f() { return P; }
template <const S *P> constexpr const S *g() { return P; }
template <S *P> constexpr S *h() { return P; }
template <const void *P> struct X { static constexpr const void *p = P; };
template <bool V> struct Z { static constexpr bool v = V; };
template <int S::*M> struct MP { static constexpr int S::*m = M; };
enum E { e1 = 1 };
struct CI { constexpr operator int() const { return 1; } };
static_assert(f<nullptr>() == nullptr);
static_assert(f<cvp>() == &s);
static_assert(g<&s>() == &s && g<&cs>() == &cs);
static_assert(h<&s>() == &s);
static_assert(X<(const void *)&s>::p == &s);
static_assert(X<static_cast<const void *>(&cs)>::p == &cs);
static_assert(X<nullptr>::p == nullptr);
static_assert(Z<1>::v && Z<e1>::v && Z<CI{}>::v && !Z<0>::v);
static_assert(MP<nullptr>::m == nullptr);
int main() { return f<cvp>() == &s && h<&s>()->v == 1 ? 0 : 1; }
