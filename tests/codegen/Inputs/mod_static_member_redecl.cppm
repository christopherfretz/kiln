// The module for ../module_static_member_redecl_importer.cpp.
export module mod_static_member_redecl;
export struct A {
  static constexpr int n = 3;
  static constexpr int r = 4;
  static const int m = 5;
  static inline int i = 6;
};
constexpr int A::r;
const int A::m;
