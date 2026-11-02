// [dcl.constexpr]/3-4, [basic.types.general]/10, [class.mi]/4 (C++26,
// P3533R2): a class with virtual bases may have constexpr constructors,
// destructors and member functions, explicitly-defaulted constexpr special
// members, and is a literal type, so its objects are created, used and
// destroyed during constant evaluation: the shared virtual base is one
// subobject, initialized by the most derived class only. The C++23
// rejections are in constexpr-virtual-base-cxx23.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-ok
//@ strict-diags
//@ std: c++26
struct V { constexpr V(int x = 0) : v(x) {} int v; };
struct L : virtual V { constexpr L() : V(1) {} };
struct R : virtual V { R() = default; constexpr ~R() {} };
struct G : virtual V { constexpr G() = default; };
struct E : virtual V { constexpr E(const E &) = default; constexpr E() : V(3) {} };
struct D : L, R { constexpr D() : V(7) {} constexpr int get() const { return v; } };
struct S : virtual L, virtual R {};

static_assert(__is_literal_type(S) && __is_literal_type(D) && __is_literal_type(G));

constexpr int build() {
  D d;
  L &l = d;
  R &r = d;
  int same = &static_cast<V &>(l) == &static_cast<V &>(r);
  S s;
  G g;
  E e;
  E e2(e);
  return same * 10000 + d.get() * 1000 + s.v * 100 + g.v * 10 + e2.v;
}
static_assert(build() == 17003);

// A constexpr variable of a class with virtual bases.
constexpr G cg;
static_assert(cg.v == 0);
