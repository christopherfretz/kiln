export module imgbic;
export struct BuiltinUser {
  int f(int x) const { return __builtin_abs(x); }
  int g;
  static constexpr double inf() { return __builtin_huge_val(); }
};
