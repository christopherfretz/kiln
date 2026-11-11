#ifndef BUILTIN_IN_CLASS_H
#define BUILTIN_IN_CLASS_H
// A builtin function first used by an inline member function: declared
// lazily, among the class's members (the shape of <typeinfo>'s
// `__builtin_strcmp` and <limits>' `__builtin_huge_val`).
struct BuiltinUser {
  int f(int x) const { return __builtin_abs(x); }
  int g;
  static constexpr double inf() { return __builtin_huge_val(); }
};
#endif
