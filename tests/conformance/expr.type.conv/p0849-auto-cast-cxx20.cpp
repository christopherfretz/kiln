// [dcl.spec.auto.general]/5 (C++20): before P0849 (C++23) `auto` is not a
// functional-cast type: clang's "'auto' not allowed here".
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 9:16 error "'auto' not allowed here"
//@ diag: 10:16 error "'auto' not allowed here"
void f(int i) {
  int j = (int)auto(i);
  int k = (int)auto{i};
  (void)j, (void)k;
}
