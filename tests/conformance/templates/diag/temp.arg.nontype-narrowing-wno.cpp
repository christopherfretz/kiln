// [temp.arg.nontype]/2, [expr.const]/12, [except.spec]/2: clang's
// narrowing diagnostic for a converted constant expression is the
// warning -Wc++11-narrowing, an error by default. Disabled, the value
// is converted and compilation goes on (S<300> is S<44>).
//@ args: -Wno-c++11-narrowing
template <unsigned char C> struct S { static constexpr int v = C; };
static_assert(S<300>::v == 44);
void f() noexcept(2);
static_assert(noexcept(f()));
enum E : unsigned char { X = 300 };
static_assert(X == 44);
