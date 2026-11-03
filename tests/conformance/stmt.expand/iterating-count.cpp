// [stmt.expand]/5.2 (P1306R5): N is the first i for which `begin + i !=
// end` is false. kiln counts a pointer or a standard library contiguous
// iterator (std::array, std::span, std::string_view, std::vector's) with
// one evaluation of `end - begin` (spec/sema.md §7.6a), which equals that
// i; any other iterator is counted incrementally, so a class whose `!=`
// disagrees with its `-` gets the comparison's count.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>
#include <span>
#include <string_view>
constexpr std::array<int, 5> arr{1, 2, 3, 4, 5};
constexpr int sum_array() {
  int s = 0;
  template for (constexpr int x : arr) { s += x; }
  return s;
}
static_assert(sum_array() == 15);
constexpr int sum_span() {
  int s = 0;
  template for (constexpr int x : std::span<const int, 5>(arr)) { s += x; }
  return s;
}
static_assert(sum_span() == 15);
constexpr std::string_view sv = "kiln";
constexpr int count_sv() {
  int n = 0;
  template for (constexpr char c : sv) { n += c == 'i' || c == 'n'; }
  return n;
}
static_assert(count_sv() == 2);
struct PtrRange {
  const int* b; const int* e;
  constexpr const int* begin() const { return b; }
  constexpr const int* end() const { return e; }
};
constexpr int raw[4] = {10, 20, 30, 40};
constexpr int sum_ptr() {
  int s = 0;
  template for (constexpr int x : PtrRange{raw + 1, raw + 4}) { s += x; }
  return s;
}
static_assert(sum_ptr() == 90);
// `!=` is false from index 2 on although `end - begin` is 5.
struct Odd {
  int i;
  constexpr Odd operator+(long n) const { return {i + int(n)}; }
  constexpr long operator-(Odd o) const { return i - o.i; }
  constexpr bool operator!=(Odd o) const { return i < 2 && i != o.i; }
  constexpr int operator*() const { return i; }
};
struct OddRange {
  constexpr Odd begin() const { return {0}; }
  constexpr Odd end() const { return {5}; }
};
constexpr int count_odd() {
  int n = 0;
  template for (constexpr int x : OddRange{}) { n += 1 + x * 0; }
  return n;
}
static_assert(count_odd() == 2);
