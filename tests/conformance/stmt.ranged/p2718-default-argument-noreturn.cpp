// [class.temporary]/7 (C++23, P2718R0), [dcl.attr.noreturn],
// [stmt.return]/4: a default argument holding a temporary used in the
// for-range-initializer is rebuilt for the use (spec/sema.md §7.6a), and
// the copy is evaluated in the caller, so the CFG walks it (WP-1166): the
// call of the `[[noreturn]]` `die` in `m`'s copy ends the path, and
// flowing off the end of `m` is unreachable (no -Wreturn-type). A default
// argument that is not rewritten (`n`, a use outside a lifetime-extending
// context) stays one element, as in clang, and so does the warning;
// `k` passes the argument. clang 19 and 22 leave the rewritten default
// argument's CFG element empty and warn in `m` too.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ skip(clang): clang 19/22 do not walk a rewritten default argument in the CFG
//@ diag: 34:1 warning "non-void function does not return a value"
//@ diag: 38:1 warning "non-void function does not return a value"
[[noreturn]] int die();
struct D {
  int v;
  ~D();
};
struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
A mkd(const D& = D{die()});
int m() {
  for (int x : mkd())
    (void)x;
}
int n() {
  mkd();
}
int k() {
  for (int x : mkd(D{1}))
    (void)x;
}
