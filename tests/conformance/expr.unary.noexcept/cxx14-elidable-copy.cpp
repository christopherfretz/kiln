// [expr.unary.noexcept], C++14 [class.copy]/31: before C++17 guaranteed
// copy elision, `a(A{})` copy-initializes the parameter from a temporary
// through the copy constructor (elidable, but still potentially invoked), so a
// `noexcept(false)` copy constructor makes the expression potentially
// throwing. Found running GCC's C++ testsuite (WP-1467).
//@ std: c++14
struct A {
  A() noexcept {}
  A(const A &) noexcept(false) {}
};
void a(A) noexcept {}
static_assert(!noexcept(a(A{})), "");
static_assert(!noexcept(A(A())), "");
