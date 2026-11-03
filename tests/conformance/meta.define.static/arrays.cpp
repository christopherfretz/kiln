// [meta.define.static] (P3491R3), spec/reflection-values.md §4:
// reflect_constant_array and define_static_array. The result is a template
// parameter object of type const T[N] whose elements are
// template-argument-equivalent to reflect_constant of each element (an
// array element by reflect_constant_array of it); equal contents give the
// same object. define_static_array returns a span over it (kiln: of
// dynamic extent, spec/reflection-values.md §4.3), and an empty range a
// null, empty span.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

constexpr int arr[] = {3, 1, 4};
int marr[] = {3, 1, 4};

// reflect_constant_array: content-interned.
static_assert(reflect_constant_array(arr) == reflect_constant_array(arr));
static_assert(reflect_constant_array(arr) == reflect_constant_array(std::span<const int>(arr, 3)));
static_assert(reflect_constant_array(std::span<const int>(arr, 2)) != reflect_constant_array(arr));
constexpr long larr[] = {3, 1, 4};
static_assert(reflect_constant_array(larr) != reflect_constant_array(arr));
static_assert(extract<const int*>(reflect_constant_array(arr))[2] == 4);
static_assert(extract<const int (&)[3]>(reflect_constant_array(arr))[1] == 1);
// Not the argument itself: a new object (constant_of(^^arr) is this one).
static_assert(extract<const int*>(reflect_constant_array(arr)) != arr);
static_assert(extract<const int*>(^^arr) == extract<const int*>(reflect_constant_array(arr)));

// define_static_array: a span over that object.
constexpr auto sp = std::define_static_array(arr);
static_assert(sp.size() == 3 && sp[0] == 3 && sp[2] == 4);
static_assert(sp.data() == extract<const int*>(reflect_constant_array(arr)));
static_assert(std::define_static_array(std::span<const int>(arr, 3)).data() == sp.data());
static_assert(std::define_static_array(std::span<const int>(arr + 1, 2))[0] == 1);
static_assert(std::define_static_array(std::span<const int>(arr + 1, 2)).data() != sp.data() + 1);
consteval int sum(std::span<const int> s) {
  int t = 0;
  for (int v : s) t += v;
  return t;
}
static_assert(sum(std::define_static_array(arr)) == 8);

// Empty ranges.
constexpr auto e = std::define_static_array(std::span<const int>());
static_assert(e.size() == 0 && e.data() == nullptr);

// A computed range: the elements are copied, the range may be transient.
consteval std::span<const int> squares(int n) {
  int* p = new int[n];
  for (int i = 0; i < n; ++i) p[i] = i * i;
  auto s = std::define_static_array(std::span<const int>(p, n));
  delete[] p;
  return s;
}
constexpr auto sq = squares(5);
static_assert(sq.size() == 5 && sq[4] == 16);
static_assert(squares(5).data() == sq.data());

// Elements of class, pointer, enum and info type.
struct P { int x, y; };
constexpr P ps[] = {{1, 2}, {3, 4}};
constexpr auto pa = std::define_static_array(ps);
static_assert(pa.size() == 2 && pa[1].x == 3 && pa[1].y == 4);
static_assert(pa.data() == std::define_static_array(ps).data());
constexpr int* ptrs[] = {&marr[0], &marr[2], nullptr};
static_assert(std::define_static_array(ptrs)[1] == marr + 2);
static_assert(std::define_static_array(ptrs)[2] == nullptr);
enum class E : char { a = 'a', b = 'b' };
constexpr E es[] = {E::b, E::a};
static_assert(std::define_static_array(es)[0] == E::b);
constexpr info is[] = {^^int, ^^P, ^^arr};
constexpr auto ia = std::define_static_array(is);
static_assert(ia.size() == 3 && ia[1] == (^^P) && ia[2] == ^^arr);
static_assert(reflect_constant_array(is) == reflect_constant_array(is));
constexpr char chars[] = {'h', 'i'};
static_assert(std::define_static_array(chars).size() == 2);
static_assert(std::define_static_array(chars).data() != std::define_static_string(chars));

// Arrays of arrays: each element by reflect_constant_array.
constexpr int grid[2][2] = {{1, 2}, {3, 4}};
static_assert(extract<const int (&)[2][2]>(reflect_constant_array(grid))[1][0] == 3);
static_assert(reflect_constant_array(grid) == reflect_constant_array(grid));
