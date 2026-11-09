// [class.ctor.general]/5, [class.dtor]/5, [dcl.type.cv]/4: a constructor
// or destructor treats the object it runs for (and its subobjects) as
// neither const nor volatile: a base constructor at offset 0 writing its own
// members, a derived constructor writing the members of a const object
// (initializing its const member first), an empty base at offset 0, a
// member subobject at offset 0 with its own constructor, and re-constructing
// the base subobject of a volatile object through its own members. Valid
// twin of constexpr-structor-volatile-subobject.cpp. WP-1101.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
#include <memory>
struct B { int k; constexpr B(int v) : k(v) { k += 1; } constexpr ~B() { k = 0; } };
struct D : B { int m; constexpr D() : B(1), m(2) { m += k; } };
constexpr int f1() { volatile D d; return 1; }
static_assert(f1() == 1);
constexpr int f2() {
  volatile D d;
  B *b = static_cast<B*>(const_cast<D*>(&d));
  std::destroy_at(b);
  std::construct_at(b, 5);
  return 2;
}
static_assert(f2() == 2);
struct E { constexpr E() {} };
struct F : E { int m; constexpr F() : E(), m(3) { m *= 2; } };
constexpr int f3() { volatile F f; const volatile F g; return 3; }
static_assert(f3() == 3);
struct G : B { const int c; int m; constexpr G() : B(4), c(5), m(c) { m += c; k = m; } };
constexpr int f4() { const G g; const volatile G h; return g.k + g.m; }
static_assert(f4() == 20);
struct H { B b; int n; constexpr H() : b(7), n(8) { n += b.k; } };
constexpr int f5() { volatile H h; return 5; }
static_assert(f5() == 5);
