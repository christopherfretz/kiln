// [dcl.type.auto.deduct]/4: the deduced type is the type of the
// initializer after the call-deduction adjustments: bit-fields deduce
// their declared type, enumerators their enumeration, sugar is kept.
//@ strict-diags
struct B { int x : 3; unsigned y : 2; } b;
enum E { E1 };
enum class F : short { F1 };
typedef long L;
L l;
auto bx = b.x;
auto by = b.y;
auto e = E1;
auto f = F::F1;
auto ll = l;
auto n = nullptr;
auto t = true;
auto s = sizeof(int);
static_assert(__is_same(decltype(bx), int) && __is_same(decltype(by), unsigned));
static_assert(__is_same(decltype(e), E) && __is_same(decltype(f), F));
static_assert(__is_same(decltype(ll), long) && __is_same(decltype(n), decltype(nullptr)));
static_assert(__is_same(decltype(t), bool) && __is_same(decltype(s), unsigned long));
