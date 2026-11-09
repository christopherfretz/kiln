// [dcl.inline]/5 is about block-scope functions only: inline namespace-scope
// functions and members, including members of a local class, are fine.
// Invalid pair: inline-block-scope-forms.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-ok
inline int ok() { return 1; }
struct S { inline int m(); inline int n() { return 2; } };
inline int S::m() { return 3; }
int f() {
  struct L { inline int m() { return 4; } };
  return L().m() + S().m() + S().n() + ok();
}
