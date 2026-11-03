// [dcl.constexpr]/3 (GNU computed goto): clang never allows an indirect
// `goto` in a constexpr function, at any language level ("statement not
// allowed in constexpr function", also in a template definition), while a
// label address is fine (its label statement is a C++23 extension before
// C++23).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:5 error "statement not allowed in constexpr function"
//@ diag: 20:3 error "indirect goto in function with no address-of-label expressions"
//@ diag: 20:3 error "statement not allowed in constexpr function"
constexpr int f(int i) {
  if (i) {
    void *p = &&l;
    goto *p;
  }
l:
  return 0;
}
template <class T> constexpr T g(void *p) {
  goto *p;
  return T();
}
constexpr int h() {
  void *p = &&m;
m:
  return p != nullptr;
}
static_assert(h() == 1, "");
