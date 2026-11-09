// [dcl.inline]/5: the inline specifier shall not appear on a block-scope
// declaration of a function. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 7:3 error "inline declaration of 'g' not allowed in block scope"
void f() {
  inline void g();
}
