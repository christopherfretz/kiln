// [expr.type]/4: the composite pointer type of two pointers whose pointees
// differ only in cv-qualification, or where one points to void or to a base
// class, is a pointer to the cv-combined type (`void *` vs `const B *` is
// `const void *`). It is used by ==, <, ?: and in constant evaluation.
//@ expect: run
//@ strict-diags
struct B {};
struct D : B {};
void *vp; const B *cbp; B *bp; const D *cdp; volatile int *vip; const void *cvp;
static_assert(__is_same(decltype(true ? vp : cbp), const void *));
static_assert(__is_same(decltype(true ? bp : cdp), const B *));
static_assert(__is_same(decltype(true ? vip : cvp), const volatile void *));
constexpr int g() { int a[2]{}; const int *p = a; void *q = a; return p == q; }
static_assert(g() == 1);
int main() {
  static D d;
  vp = &d; cbp = &d; bp = &d; cdp = &d;
  int i = 0; vip = &i; cvp = &i;
  bool ok = vp == cbp && bp == cdp && !(cdp != bp) && vip == cvp && !(cbp < vp);
  return ok ? 0 : 1;
}
