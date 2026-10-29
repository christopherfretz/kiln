// [dcl.type.auto.deduct]/4: `auto *` deduces the pointee, including from
// pointers to functions and members.
//@ strict-diags
struct S { int m; int f(); };
int i;
const int *cp = &i;
auto *a = &i;
auto *b = cp;
const auto *c = &i;
auto **d = &a;
auto f = &S::m;
auto g = &S::f;
static_assert(__is_same(decltype(a), int *) && __is_same(decltype(b), const int *));
static_assert(__is_same(decltype(c), const int *) && __is_same(decltype(d), int **));
static_assert(__is_same(decltype(f), int S::*) && __is_same(decltype(g), int (S::*)()));
