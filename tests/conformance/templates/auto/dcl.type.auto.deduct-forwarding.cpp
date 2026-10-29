// [dcl.type.auto.deduct]/4, [temp.deduct.call]/3: `auto &&` is a
// forwarding reference: an lvalue initializer deduces an lvalue reference.
//@ strict-diags
int i;
const int ci = 1;
int &&xv();
auto &&a = i;
auto &&b = ci;
auto &&d = xv();
int &&xv2();
auto &&e = xv2();
static_assert(__is_same(decltype(a), int &));
static_assert(__is_same(decltype(b), const int &));
static_assert(__is_same(decltype(d), int &&));
static_assert(__is_same(decltype(e), int &&));
