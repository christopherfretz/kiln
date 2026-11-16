//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ requires: clang>=23
//@ stdout: 15 6 2 21
//@ stdout: 2 1
// [stmt.expand]/5.2 as amended by CWG3140 (working draft): the iterating
// form of a for-range-declaration without constexpr declares
// `decltype(auto) range = (init); auto begin = begin-expr;`, neither
// constexpr, and N is the distance from begin-expr to end-expr evaluated
// in a consteval lambda capturing range by reference, so a range that is
// not a constant expression is fine when its size is (range is a
// reference of unknown referent there, P2280): a std::array returned by a
// non-constexpr function, a function parameter, a class whose begin/end
// ignore the object. This is the only form of such a statement (WP-1483:
// no P1306R5 constant copy is tried first). Found running GCC's C++
// testsuite.
#include <array>
extern "C" int printf(const char *, ...);

std::array<int, 5> make() { return {1, 2, 3, 4, 5}; }

int sum_param(std::array<int, 3> a) {
  int s = 0;
  template for (auto x : a) { s += x; }
  return s;
}

struct It {
  int x;
  constexpr It &operator++() { ++x; return *this; }
  constexpr int operator*() const { return x; }
  constexpr bool operator!=(const It &o) const { return x != o.x; }
  constexpr It operator+(long o) const { return It{x + int(o)}; }
  constexpr long operator-(const It &o) const { return x - o.x; }
};
struct Fixed { int unused; };
constexpr It begin(const Fixed &) { return It{1}; }
constexpr It end(const Fixed &) { return It{7}; }

int main() {
  int r = 0;
  template for (auto n : make()) { r += n; }
  int k = 0;
  template for (auto n : Fixed{42}) { k += n; }
  int c = 0;
  std::array<int, 2> m{1, 1};
  template for (auto &n : m) { n = ++c; }
  printf("%d %d %d %d\n", r, sum_param({1, 2, 3}), c, k);
  printf("%d %d\n", m[1], m[0]);
}
