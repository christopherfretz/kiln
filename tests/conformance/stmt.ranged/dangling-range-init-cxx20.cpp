// [stmt.ranged]/1, [class.temporary]/6 (WP-875): before C++23 (P2718) a
// temporary in the range-initializer that is not the one bound to the
// implicit __range reference is destroyed at the end of the
// full-expression; clang warns on the implicit variable ("implicitly bound
// to local reference"). Directly bound temporaries are extended: no warning.
//@ strict-diags
//@ std: c++20
//@ expect: compile-ok
//@ diag: 19:20 warning "temporary implicitly bound to local reference will be destroyed at the end of the full-expression"
//@ diag: 21:16 warning "temporary implicitly bound to local reference will be destroyed at the end of the full-expression"

struct V { int v[2]; int *begin(); int *end(); };
struct W { const int &r; const int *begin() const; const int *end() const; };
struct R { const int *b, *e; const int *begin() const; const int *end() const; };
struct H { R r; const R &get() const [[clang::lifetimebound]] { return r; } };
const R &rid(const R &x [[clang::lifetimebound]]);
int g;
void h() {
  for (int x : rid(R{})) (void)x;
  for (int x : W{5}) (void)x;
  for (int x : H{}.get()) (void)x;
  for (int x : V{}) (void)x;
  for (int x : H().r) (void)x;
  for (int x : W{g}) (void)x;
}
