// [cpp.pragma] Every declaration clang's ProcessDeclAttributes handles
// takes the pushed '#pragma clang attribute' attributes: friend functions
// (declared only, defined, a hidden friend of a class template and its
// instantiation), explicit specializations of function and variable
// templates, a lambda's call operator, a defaulted constructor, and the
// variable of a structured binding declaration. spec/sema.md
// "#pragma clang attribute".
//@ expect: ir
struct P { int a, b; };
template <class T> int vt = 0;
template <class T> int ft(T) { return 0; }
#pragma clang attribute push (__attribute__((cold)), apply_to = function)
template <> int ft<int>(int) { return 1; }
struct S {
  friend int fr(S) { return 2; }
  friend int decl_only(S);
  S() = default;
};
template <class T> struct TS {
  friend int hf(TS) { return 3; }
};
int lam() {
  return [] { return 4; }();
}
#pragma clang attribute pop
int decl_only(S) { return 5; }

#pragma clang attribute push (__attribute__((annotate("v"))), apply_to = variable(unless(is_parameter)))
template <> int vt<int> = 1;
int locals() {
  auto [x, y] = P{1, 2};
  try {
  } catch (int e) {
    return e;
  }
  return x;
}
#pragma clang attribute pop
int g() { return vt<int> + ft(0) + fr(S{}) + hf(TS<int>{}) + decl_only(S{}); }
