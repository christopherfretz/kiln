// [dcl.type.auto.deduct]/4, [temp.deduct.call]/2: a function initializer
// decays to a pointer for `auto`; `auto &` binds the function itself.
//@ strict-diags
void g(int);
int h() noexcept;
auto p = g;
auto &r = g;
auto *q = h;
const auto c = &g;
static_assert(__is_same(decltype(p), void (*)(int)));
static_assert(__is_same(decltype(r), void (&)(int)));
static_assert(__is_same(decltype(q), int (*)() noexcept));
static_assert(__is_same(decltype(c), void (*const)(int)));
