// [over.match.list]/1: phase 1 considers initializer-list constructor
// templates (`initializer_list<U>` deduced from the list); a class with
// an initializer-list constructor and constructor templates falls back
// to phase 2 when phase 1 finds nothing viable, and a nested
// braced-init-list is a phase-2 constructor argument. [over.ics.rank]/3.1:
// a list converting to std::initializer_list beats one converting to an
// array reference (libc++'s C++26 `span(initializer_list)` against
// `span(element_type (&)[N])`).
//@ expect: run
//@ strict-diags
//@ exit: 0
#include <initializer_list>
struct P { int x; };
struct S {
  int k = 0;
  constexpr S(std::initializer_list<int> l) : k(100 + (int)l.size()) {}
  template<class T> constexpr S(T*, T*) : k(2) {}
  constexpr S(const char*) : k(3) {}
};
constexpr S a{"hi"};
static_assert(a.k == 3);
constexpr int arr[2] = {};
constexpr S b{arr, arr + 1};
static_assert(b.k == 2);
constexpr S c{1, 2, 3};
static_assert(c.k == 103);
struct T {
  int k = 0;
  template<class U> constexpr T(std::initializer_list<U> l) : k(200 + (int)l.size()) {}
  constexpr T(P) : k(5) {}
};
constexpr T d{1, 2};
static_assert(d.k == 202);
constexpr T e{P{1}};
static_assert(e.k == 201);
struct V {
  int k = 0;
  constexpr V(std::initializer_list<int> l) : k((int)l.size()) {}
  constexpr V(const int (&)[2]) : k(-1) {}
};
constexpr V v({7, 8});
static_assert(v.k == 2);
struct W {
  int k = 0;
  constexpr W(const V& x) : k(10 + x.k) {}
  template<class X> constexpr W(X*) : k(1) {}
  constexpr W(std::initializer_list<P>) : k(-2) {}
};
constexpr W w{{1, 2, 3}};
static_assert(w.k == 13);
int main() {
  S rs{1, 2};
  T rt{3, 4, 5};
  V rv({1, 2});
  W rw{{4}};
  // `{4}` initializes a P: phase 1 selects the initializer_list<P>.
  return rs.k + rt.k + rv.k + rw.k == 102 + 203 + 2 - 2 ? 0 : 1;
}
