// A header made a system header by `#pragma GCC system_header`, shaped
// like libstdc++'s <cmath> overloads: clang does not run the never-constant
// check on its functions, even under -Wsystem-headers (WP-945).
#pragma GCC system_header
inline constexpr float sys_acos(float x) { return __builtin_acosf(x); }
inline constexpr double sys_floor() { return __builtin_floor(2.5); }
constexpr int sys_asm(int x) {
  asm("");
  return x;
}
