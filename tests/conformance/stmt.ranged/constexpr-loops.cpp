// [stmt.ranged] in constant evaluation ([expr.const]): array, member,
// argument-dependent and braced-init-list ranges, an init-statement,
// references that modify the range, and `break`/`continue`.
#include <initializer_list>

struct R {
  int a[3] = {1, 2, 3};
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 3; }
};
namespace N {
struct S { int v[2] = {4, 5}; };
constexpr const int* begin(const S& s) { return s.v; }
constexpr const int* end(const S& s) { return s.v + 2; }
}

constexpr int arrays() {
  int a[] = {1, 2, 3, 4};
  int s = 0;
  for (int x : a) s += x;
  return s;
}
static_assert(arrays() == 10);

constexpr int members() { int s = 0; for (auto x : R{}) s += x; return s; }
static_assert(members() == 6);

constexpr int adl() { int s = 0; for (int x : N::S{}) s += x; return s; }
static_assert(adl() == 9);

constexpr int lists() { int s = 0; for (int x : {1, 2, 3}) s += x; return s; }
static_assert(lists() == 6);

constexpr int init() { int s = 0; for (int a[] = {5, 6}; int x : a) s += x; return s; }
static_assert(init() == 11);

constexpr int modify() {
  int a[] = {1, 2, 3};
  for (auto& x : a) x *= 2;
  int s = 0;
  for (const auto& x : a) s += x;
  return s;
}
static_assert(modify() == 12);

constexpr int jumps() {
  int a[] = {1, 2, 3, 4, 5};
  int s = 0;
  for (int x : a) {
    if (x == 2) continue;
    if (x == 4) break;
    s += x;
  }
  return s;
}
static_assert(jumps() == 4);

constexpr int nested() {
  int m[2][2] = {{1, 2}, {3, 4}};
  int s = 0;
  for (auto& row : m)
    for (int x : row) s = s * 10 + x;
  return s;
}
static_assert(nested() == 1234);
