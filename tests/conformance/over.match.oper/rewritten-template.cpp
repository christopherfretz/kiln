// [over.match.oper]/3.4, [temp.deduct.call]: operator function templates
// are rewritten (`!=` from `==`, `<` from `<=>`) and reversed candidates,
// deduced against the operands in their parameter order; partial ordering
// reverses one candidate's parameters ([temp.func.order]/3).
//@ expect: compile-ok
struct A { int v; };
template <class T> constexpr bool operator==(const A& a, const T& t) { return a.v == t; }
static_assert(A{3} == 3);
static_assert(3 == A{3});
static_assert(A{3} != 4);
static_assert(4 != A{3});
struct B {
  int v;
  template <class T> constexpr bool operator==(const T& t) const { return v == t; }
};
static_assert(B{2} == 2);
static_assert(2 == B{2});
static_assert(2 != B{3});
struct C {
  int v;
  template <class T> constexpr int operator<=>(const T& t) const { return v - t; }
  template <class T> constexpr bool operator==(const T& t) const { return v == t; }
};
static_assert(C{1} < 2);
static_assert(2 > C{1});
static_assert(0 <= C{1});
template <class T> struct W { T v; };
template <class T, class U> constexpr bool operator==(const W<T>& a, const W<U>& b) { return a.v == b.v; }
static_assert(W<int>{1} != W<long>{2});
static_assert(W<int>{1} == W<long>{1});
struct D {};
template <class T> constexpr int operator==(D, T*) { return 1; }
template <class T> constexpr int operator==(T*, D) { return 2; }
constexpr int* np = nullptr;
static_assert((D{} == np) == 1 && (np == D{}) == 2);
struct E {};
template <class T> constexpr bool operator==(E, T) { return false; }
constexpr bool operator==(int, E) { return true; }
static_assert(1 == E{});
static_assert(E{} == 1);  // the reversed non-template wins
struct F {};
template <class T> constexpr int operator<=>(F, T) { return -1; }
static_assert(F{} < 1 && 1 > F{});
