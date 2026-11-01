// [basic.lookup.qual.general]/1, [dcl.typedef]/2: a typedef-name that
// names a class or enumeration may be the qualifier of a type at the
// start of a declaration, also for members inherited from a base
// (libc++ <typeinfo>: `typedef __type_info_implementations::__impl __impl;
// __impl::__type_name_t __type_name;`).
//@ strict-diags
namespace impls {
struct B { typedef const char* T; };
struct D : B { typedef int U; };
typedef D I;
}
enum class E { a };
typedef impls::D J;
using K = E;
J::U g1;
J::T g2;
K g3 = K::a;
class C {
protected:
  typedef impls::I I;
  I::T x;
  I::U y;
};
struct S { typedef J L; L::T z; };
void f() {
  typedef impls::D M;
  M::T w = nullptr;
  M::U v = 0;
  (void)w; (void)v;
}
static_assert(__is_same(J::T, const char*));
static_assert(__is_same(decltype(S::z), const char*));
