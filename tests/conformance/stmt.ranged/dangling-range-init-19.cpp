// [stmt.ranged]/1, [class.temporary]/6.6 (WP-875): clang 19 implements
// P2718 lifetime extension of range-initializer temporaries but still warns
// on the implicit __range variable even in C++23 and later; clang 22 does not
// (the sibling dangling-range-init.cpp). C++20: dangling-range-init-cxx20.cpp.
//@ strict-diags
//@ requires: clang<22
//@ expect: compile-ok
//@ diag: 18:20 warning "temporary implicitly bound to local reference will be destroyed at the end of the full-expression"
//@ diag: 20:16 warning "temporary implicitly bound to local reference will be destroyed at the end of the full-expression"

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
