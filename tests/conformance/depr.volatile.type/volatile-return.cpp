// [depr.volatile.type]/2: a volatile-qualified return type is deprecated
// since C++20 (-Wdeprecated-volatile), at the declarator's '(' or a
// trailing return type's `auto`; a pointer to volatile is not.
//@ std: c++20
//@ strict-diags
//@ diag: 12:15 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 13:21 warning "volatile-qualified return type 'const volatile int' is deprecated"
//@ diag: 16:5 warning "volatile-qualified return type 'VI' (aka 'volatile int') is deprecated"
//@ diag: 17:1 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 18:26 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 19:12 warning "volatile-qualified return type 'volatile int' is deprecated"
volatile int f();
const volatile int g();
volatile int *h();
typedef volatile int VI;
VI i();
auto j() -> volatile int;
struct S { volatile int m(); };
auto l = []() -> volatile int { return 0; };
int main() {}
