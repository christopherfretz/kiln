// [over.match.oper]/3.4: which rewritten candidate is selected.
//@ expect: compile-ok
// [over.match.oper]/3.4: rewritten candidates.
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
struct A { int v; bool operator==(int) const; };
// `1 == a` uses the reversed `a == 1`; `a != 1` is `!(a == 1)`.
static_assert(same<decltype(1 == A()), bool>::value);
static_assert(same<decltype(A() != 1), bool>::value);
static_assert(same<decltype(1 != A()), bool>::value);
struct O { int operator<=>(const O &) const; };
struct P { int operator<=>(int) const; };
// Relational operators are rewritten to `(x <=> y) @ 0`, or reversed to
// `0 @ (y <=> x)`.
static_assert(same<decltype(O() < O()), bool>::value);
static_assert(same<decltype(P() >= 1), bool>::value);
static_assert(same<decltype((1 > P())), bool>::value);
// A non-rewritten candidate is better than a rewritten one
// ([over.match.best]/2.8).
struct Q { bool operator==(const Q &) const; char operator!=(const Q &) const; };
static_assert(same<decltype(Q() != Q()), char>::value);
// P2468: an `operator==` with a matching `operator!=` is not rewritten.
struct M { bool operator==(int) const; bool operator!=(int) const; };
bool m1(M m) { return m == 1; }
