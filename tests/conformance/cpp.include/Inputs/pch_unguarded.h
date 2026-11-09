// A precompiled-header prefix with no include guard and no '#pragma once'
// (the shape CMake's target_precompile_headers wraps): entering it twice
// would redefine everything below.
struct PchPoint {
  int x, y;
  constexpr int sum() const { return x + y; }
};
template <class T> constexpr T pch_twice(T v) { return v + v; }
inline constexpr int pch_value = pch_twice(21);
#define PCH_MACRO(a) ((a) * 3)
