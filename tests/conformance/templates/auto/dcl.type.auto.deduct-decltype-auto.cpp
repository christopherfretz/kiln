// [dcl.type.auto.deduct]/5: decltype(auto) deduces decltype(e) of the
// initializer, with the id-expression and parenthesized forms.
//@ strict-diags
int i;
const int ci = 1;
int &&xv();
int &lv();
struct S { int m; } s;
decltype(auto) a = i;
decltype(auto) b = (i);
decltype(auto) c = ci;
decltype(auto) d = xv();
decltype(auto) e = lv();
decltype(auto) f = 1 + 2;
decltype(auto) g = s.m;
decltype(auto) h = (s.m);
static_assert(__is_same(decltype(a), int) && __is_same(decltype(b), int &));
static_assert(__is_same(decltype(c), const int) && __is_same(decltype(d), int &&));
static_assert(__is_same(decltype(e), int &) && __is_same(decltype(f), int));
static_assert(__is_same(decltype(g), int) && __is_same(decltype(h), int &));
