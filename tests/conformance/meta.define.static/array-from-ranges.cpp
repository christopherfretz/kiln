// [meta.reflection.array]: reflect_constant_array takes any input range
// and reflects the elements it iterates: a span of static extent (a
// pointer and no size member) and a range that is not contiguous (a zip
// of two arrays, transformed) included. libstdc++'s <meta> passes the
// range straight to the intrinsic (kiln_meta.h's wrapper copies a range
// without data() first), so kiln's intrinsic is called directly here.
// Found running GCC's C++ testsuite.
//@ args: -stdlib=libstdc++ -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
//@ xfail(kiln): WP-1482, the intrinsic reads a range from its layout (pointer + size, element pointers), never iterates it
#include <ranges>
#include <span>
#include <utility>
#ifdef __KILN__
#include "kiln_meta.h"
#define REFLECT_ARRAY(...) std::meta::__kiln_reflect_constant_array(__VA_ARGS__)
#else
#include <meta>
#define REFLECT_ARRAY(...) std::meta::reflect_constant_array(__VA_ARGS__)
#endif
using namespace std::meta;

constexpr int arr[4]{1, 2, 3, 4};
constexpr info r = REFLECT_ARRAY(std::span<const int, 4>(arr));
static_assert(r == REFLECT_ARRAY(arr));

constexpr int x[]{1, 2, 3};
constexpr int y[]{11, 12, 13};
constexpr auto as_pair = [](const auto &t) {
  return std::make_pair(std::get<0>(t), std::get<1>(t));
};
constexpr info zr = REFLECT_ARRAY(std::views::zip(x, y) | std::views::transform(as_pair));
static_assert(extract<const std::pair<int, int> *>(zr)[1].second == 12);
