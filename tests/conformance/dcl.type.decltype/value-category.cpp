// [dcl.type.decltype]/1.4-1.6: otherwise xvalue gives `T&&`, lvalue `T&`,
// prvalue `T`.
//@ strict-diags
int f();
int &g();
int &&h();
const int k();
int i;
int a[3];
static_assert(__is_same(decltype(f()), int));
static_assert(__is_same(decltype(g()), int &));
static_assert(__is_same(decltype(h()), int &&));
static_assert(__is_same(decltype(k()), int));
static_assert(__is_same(decltype(i + 1), int));
static_assert(__is_same(decltype(*&i), int &) && __is_same(decltype(a[0]), int &));
static_assert(__is_same(decltype("ab"), const char (&)[3]));
static_assert(__is_same(decltype(1, i), int &) && __is_same(decltype(i, 1), int));
static_assert(__is_same(decltype(true ? i : i), int &) && __is_same(decltype(true ? i : 1), int));
