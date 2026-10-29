// [dcl.type.auto.deduct]/4: direct-list-initialization from a single
// element deduces that element's type ([dcl.init.list]/3).
//@ strict-diags
auto a{1};
auto b{2.5f};
const auto c{'c'};
int i;
auto *d{&i};
static_assert(__is_same(decltype(a), int) && __is_same(decltype(b), float));
static_assert(__is_same(decltype(c), const char) && __is_same(decltype(d), int *));
