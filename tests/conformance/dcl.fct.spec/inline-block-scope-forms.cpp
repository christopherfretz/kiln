// [dcl.inline]/5: the inline specifier is not used in a block-scope
// function declaration, `extern` or not, also in a lambda body; reported
// at `inline`. Valid pair: inline-block-scope-valid.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 11:3 error "inline declaration of 'g' not allowed in block scope"
//@ diag: 12:10 error "inline declaration of 'h' not allowed in block scope"
//@ diag: 13:17 error "inline declaration of 'q' not allowed in block scope"
//@ diag: 15:32 error "inline declaration of 'r' not allowed in block scope"
void f() {
  inline void g();
  extern inline int h(int);
  auto l = [] { inline void q(); };
}
template <class T> void tf() { inline T r(int); }
