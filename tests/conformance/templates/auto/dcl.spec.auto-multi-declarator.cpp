// [dcl.spec.auto.general]/8: each declarator deduces separately and the
// deduced types must agree.
//@ strict-diags
int i;
auto a = 1, *b = &i, &c = i;
const auto d = 1, e = 2;
static_assert(__is_same(decltype(a), int) && __is_same(decltype(b), int *));
static_assert(__is_same(decltype(c), int &) && __is_same(decltype(e), const int));
