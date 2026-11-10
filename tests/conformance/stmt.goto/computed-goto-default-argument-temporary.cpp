// [stmt.goto] (GNU computed goto), [class.temporary]/7 (C++23, P2718R0),
// [intro.execution]/5: a default argument's temporary used in the
// for-range-initializer persists until the end of the loop, through a
// copy of the default argument rebuilt for the use (spec/sema.md §7.6a),
// so an indirect `goto` out of the loop leaves the scope of a
// lifetime-extended temporary with a non-trivial destructor, as when the
// temporary is written in the call (`g`). The jump-scope walk enters the
// copy (WP-1166); a use outside a lifetime-extending context (`h`) is
// destroyed at the end of its full-expression and opens no scope. clang
// 19 and 22 do not walk the rewritten default argument: they accept `f`
// and skip the destructor.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ skip(clang): clang 19/22 do not walk a rewritten default argument in the jump-scope checker
//@ diag: 37:7 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 38:1 note "possible target of indirect goto statement"
//@ diag: 31:18 note "jump exits scope of lifetime-extended temporary with non-trivial destructor"
//@ diag: 44:7 error "cannot jump from this indirect goto statement to one of its possible targets"
//@ diag: 45:1 note "possible target of indirect goto statement"
//@ diag: 42:21 note "jump exits scope of lifetime-extended temporary with non-trivial destructor"
struct D {
  int v;
  ~D();
};
struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
A mk(const D& = D{7});
A mk2(const D&);
void f(int n) {
  static void* t[] = {&&out};
  for (int x : mk())
    if (x == n)
      goto* t[0];
out:;
}
void g(int n) {
  static void* t[] = {&&out};
  for (int x : mk2(D{7}))
    if (x == n)
      goto* t[0];
out:;
}
void h(int n) {
  static void* t[] = {&&out};
  mk();
  if (n)
    goto* t[0];
out:;
}
