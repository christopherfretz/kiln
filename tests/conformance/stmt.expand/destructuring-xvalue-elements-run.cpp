// [stmt.expand]/5.3 (CWG3149): in a destructuring expansion statement
// whose expansion-initializer is not an lvalue, the element initializing
// the for-range-declaration is `static_cast<decltype(u_i)&&>(u_i)`, an
// xvalue for a non-reference element, so `auto&&` deduces an rvalue
// reference (a tuple-like element of lvalue reference type stays one, an
// rvalue reference one stays one); an lvalue expansion-initializer gives
// the bindings themselves, lvalues. A move-only element can initialize an
// `auto` by move, in a function template as well. Found running GCC's C++
// testsuite.
//@ skip(clang): clang 22 does not implement expansion statements
//@ oracle: gcc
//@ expect: run
#include <tuple>
#include <type_traits>
struct MoveOnly {
  int v;
  MoveOnly(int v) : v(v) {}
  MoveOnly(MoveOnly &&o) : v(o.v) { o.v = -1; }
};
struct P { int a; long b; };
long g = 3;
std::tuple<int, long &, int &&> tup();
int k = 4;
std::tuple<int, long &, int &&> tup() { return {1, g, static_cast<int &&>(k)}; }
template <class T> int sum_moved() {
  int s = 0;
  template for (auto e : std::tuple<T, T>(T(1), T(2))) { s += e.v; }
  return s;
}
int main() {
  int bad = 0;
  template for (auto &&e : tup()) {
    bad += !std::is_rvalue_reference_v<decltype(e)> && !std::is_same_v<decltype(e), long &>;
  }
  template for (auto &&e : P{}) { bad += !std::is_rvalue_reference_v<decltype(e)>; }
  P lv{};
  template for (auto &&e : lv) { bad += !std::is_lvalue_reference_v<decltype(e)>; }
  bad += sum_moved<MoveOnly>() != 3;
  return bad;
}
