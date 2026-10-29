// [dcl.type.auto.deduct]/5: `decltype(auto)` return types deduce as
// `decltype(e)` of the returned operand.
//@ strict-diags
int i;
int &&xv();
decltype(auto) a() { return i; }
decltype(auto) b() { return (i); }
decltype(auto) c() { return xv(); }
decltype(auto) d() { return 1 + 1; }
decltype(auto) e() {}
static_assert(__is_same(decltype(a()), int));
static_assert(__is_same(decltype(b()), int &));
static_assert(__is_same(decltype(c()), int &&));
static_assert(__is_same(decltype(d()), int));
static_assert(__is_same(decltype(e()), void));
