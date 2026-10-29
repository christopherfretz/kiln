// [dcl.type.auto.deduct]/4: cv-qualifiers and & on the placeholder are
// kept; `auto &` keeps the initializer's cv.
//@ strict-diags
int i;
const int ci = 1;
const auto a = i;
auto &b = ci;
const auto &c = i;
volatile auto d = 1;
auto &e = i;
auto *const f = &i;
static_assert(__is_same(decltype(a), const int));
static_assert(__is_same(decltype(b), const int &));
static_assert(__is_same(decltype(c), const int &));
static_assert(__is_same(decltype(d), volatile int));
static_assert(__is_same(decltype(e), int &));
static_assert(__is_same(decltype(f), int *const));
