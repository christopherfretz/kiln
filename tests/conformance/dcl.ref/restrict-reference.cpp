// GNU `__restrict` / `__restrict__` after `&` or `&&` in a declarator
// (WP-525; libstdc++ 16 <cxxabi.h> `__upcast_result& __restrict __result`
// parameters): the qualifier is accepted and dropped, as
// references cannot be cv- or restrict-qualified objects ([dcl.ref]/1; clang
// `ParseDeclaratorInternal`). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
struct R { int v = 1; };
struct C {
  int f(const R& __restrict r) const { return r.v; }
  int g(R& __restrict__ r, int* __restrict q) { return r.v + *q; }
};
int h(int&& __restrict x) { return x; }
int k() {
  R r;
  int i = 2;
  int& __restrict j = i;
  return C{}.f(r) + C{}.g(r, &j) + h(3);
}
