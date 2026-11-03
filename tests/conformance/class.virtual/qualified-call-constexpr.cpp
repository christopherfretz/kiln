// [class.virtual]/16: explicit qualification with the scope operator
// suppresses the virtual call mechanism, also on an explicit member access
// (`r.A::f()`, `p->A::f()`) in a constant expression; [expr.typeid]/1,
// [expr.const]/13: a pointer to a type_info object is a permitted result.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
#include <typeinfo>
struct A { constexpr virtual int f() const { return 1; } };
struct B : A { constexpr int f() const override { return 2; } };
constexpr int t() {
  B b;
  const A &r = b;
  const A *p = &b;
  return r.A::f() * 1000 + p->A::f() * 100 + r.f() * 10 + b.A::f();
}
static_assert(t() == 1121);
constexpr const std::type_info *ti = &typeid(int);
static_assert(ti == &typeid(int));
