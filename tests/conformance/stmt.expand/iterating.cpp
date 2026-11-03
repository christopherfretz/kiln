// [stmt.expand]/5.2: an expansion-initializer whose type has member (or
// ADL) begin/end is an iterating expansion statement: the range, begin and
// end are constexpr variables, N is counted at compile time and element i
// is `*(begin + i)`. A local constexpr range works (the paper's example);
// a range that is not a constant expression is diagnosed.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>

consteval int paper() {
  constexpr std::array<int, 3> arr{1, 2, 3};
  int result = 0;
  template for (constexpr int s : arr) { result += sizeof(char[s]); }
  return result;
}
static_assert(paper() == 6);

constexpr std::array<int, 4> g{1, 2, 3, 4};
constexpr int global() {
  int s = 0;
  template for (constexpr int x : g) { s = s * 10 + x; }
  return s;
}
static_assert(global() == 1234);

// A prvalue range, and a non-constexpr element variable.
constexpr int prvalue() {
  int s = 0;
  template for (int x : std::array<int, 3>{4, 5, 6}) { s += x; }
  return s;
}
static_assert(prvalue() == 15);

// ADL begin/end on a literal class.
namespace lib {
struct Bag { int v[3]; };
constexpr const int* begin(const Bag& b) { return b.v; }
constexpr const int* end(const Bag& b) { return b.v + 3; }
}  // namespace lib
constexpr lib::Bag bag{{7, 8, 9}};
constexpr int adl() {
  int s = 0;
  template for (constexpr int x : bag) { s += x; }
  return s;
}
static_assert(adl() == 24);

// Elements are distinct types only through the element values: N follows
// the range, and an empty range expands to nothing.
constexpr std::array<int, 0> none{};
constexpr int empty() {
  int s = 1;
  template for (constexpr int x : none) { s += x; }
  return s;
}
static_assert(empty() == 1);
