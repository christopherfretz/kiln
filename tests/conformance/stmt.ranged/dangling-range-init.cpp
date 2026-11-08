// [stmt.ranged]/1, [class.temporary]/6.6 (P2718, WP-875): since C++23 every
// temporary in the range-initializer lives until the end of the loop, so
// nothing dangles and clang 22 does not warn; clang 19 still does (the
// sibling dangling-range-init-19.cpp).
//@ strict-diags
//@ requires: clang>=22
//@ expect: compile-ok

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
