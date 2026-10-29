// [dcl.type.auto.deduct]/4: a placeholder variable deduces as a function
// template parameter from its initializer; top-level cv and references of
// the initializer are dropped.
//@ strict-diags
int i;
const int ci = 1;
int &ri = i;
const int &cri = i;
auto a = i;
auto b = ci;
auto c = ri;
auto d = cri;
auto e = 1.5;
auto f = 'x';
auto g = i + 1L;
static_assert(__is_same(decltype(a), int) && __is_same(decltype(b), int));
static_assert(__is_same(decltype(c), int) && __is_same(decltype(d), int));
static_assert(__is_same(decltype(e), double) && __is_same(decltype(f), char));
static_assert(__is_same(decltype(g), long));
