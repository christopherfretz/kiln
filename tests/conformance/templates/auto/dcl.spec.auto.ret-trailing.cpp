// [dcl.fct]/2, [dcl.spec.auto.general]/2: a trailing return type names the
// return type; `auto` is then not deduced.
//@ strict-diags
auto a() -> int { return 1; }
auto b() -> decltype(1.0) { return 1; }
auto c() -> int (*)(int) { return nullptr; }
auto d() -> auto { return 'x'; }
auto e() -> decltype(auto) { static int i; return (i); }
static_assert(__is_same(decltype(a()), int));
static_assert(__is_same(decltype(b()), double));
static_assert(__is_same(decltype(c()), int (*)(int)));
static_assert(__is_same(decltype(d()), char));
static_assert(__is_same(decltype(e()), int &));
