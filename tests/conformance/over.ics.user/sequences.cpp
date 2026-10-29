// [over.ics.user], [over.best.ics]/4: a user-defined conversion sequence is a
// standard conversion, one converting constructor or conversion function, and
// a second standard conversion; two user-defined conversions never chain.
// Sequences with the same conversion function compare by their second
// standard conversion ([over.ics.rank]/3.3).
//@ expect: compile-ok
template <class T, class U> constexpr bool same = __is_same(T, U);
struct T1 {}; struct T2 {}; struct T3 {};
struct A { A(int); };
struct C { operator int() const; };
struct W { W(A); };
T1 f(A);
T2 f(...);
static_assert(same<decltype(f(1)), T1>);
static_assert(same<decltype(f('c')), T1>);  // promotion, then the constructor
static_assert(same<decltype(f(C())), T2>);  // C -> int -> A: two conversions
T1 w(W);
T2 w(...);
static_assert(same<decltype(w(1)), T2>);
static_assert(same<decltype(w(A(1))), T1>);

T1 g(long);
T2 g(int);
static_assert(same<decltype(g(C())), T2>);  // same function, exact after
T1 h(double);
T2 h(...);
static_assert(same<decltype(h(C())), T1>);  // standard beats ellipsis

struct P { operator int *(); };
T1 bp(bool);
T2 bp(...);
static_assert(same<decltype(bp(P())), T1>);
struct X { X(const char *); };
T1 sx(X);
T2 sx(bool);
static_assert(same<decltype(sx("s")), T2>); // standard beats user-defined
