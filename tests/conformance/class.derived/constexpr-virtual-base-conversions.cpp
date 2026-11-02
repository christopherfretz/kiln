// [conv.ptr]/3, [expr.static.cast]/11, [expr.dynamic.cast]/8,
// [class.member.lookup], [class.mi]/4 (C++26, P3533R2): during constant
// evaluation a derived-to-virtual-base conversion goes through the complete
// object, so every path to a shared virtual base designates the one
// subobject, also when the virtual base is reached through another virtual
// base or a non-virtual base of one; static_cast down to a class between
// the virtual base and the complete object, member access through each
// path, and dynamic_cast down and across through virtual bases (the
// complete object, an array element member, a heap object). The C++23
// rejection is in constexpr-virtual-base-conversions-cxx23.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-ok
//@ strict-diags
//@ std: c++26
struct V {
  int v = 1;
  constexpr virtual int f() const { return v; }
  constexpr virtual ~V() = default;
};
struct W : virtual V { int w = 2; constexpr int f() const override { return 20 + v; } };
struct X : virtual V { int x = 3; };
struct Y : virtual W, virtual X { int y = 4; };
struct Z : virtual Y, virtual X { int z = 5; };
struct M { int pad; Y y; Z zs[2]; };

constexpr bool paths() {
  Z z;
  V &v1 = static_cast<W &>(z);
  V &v2 = static_cast<X &>(z);
  V &v3 = z;
  Y &y = z;
  X &x1 = y;
  X &x2 = z;
  return &v1 == &v2 && &v2 == &v3 && &x1 == &x2 && static_cast<W *>(&y) == &static_cast<W &>(z);
}
static_assert(paths());

constexpr int members() {
  Z z;
  static_cast<X &>(z).v = 6;
  return z.v * 100000 + static_cast<W &>(z).v * 10000 + z.w * 1000 + z.x * 100 + z.y * 10 + z.z;
}
static_assert(members() == 662345);

constexpr int dispatch() {
  Z z;
  V *p = static_cast<X *>(&z);
  return p->f() * 100 + z.f();
}
static_assert(dispatch() == 2121);

constexpr int casts() {
  M m{};
  m.zs[1].v = 9;
  V *p = &m.zs[1];
  int r = p->f();
  r += (dynamic_cast<void *>(p) == static_cast<void *>(&m.zs[1])) * 100;
  r += (dynamic_cast<Y *>(p) == static_cast<Y *>(&m.zs[1])) * 1000;
  r += (dynamic_cast<W *>(static_cast<X *>(&m.y)) == static_cast<W *>(&m.y)) * 10000;
  r += (dynamic_cast<Z *>(static_cast<V *>(&m.y)) == nullptr) * 100000;
  return r;
}
static_assert(casts() == 111129);

constexpr bool heap() {
  V *p = new Z;
  Z *z = dynamic_cast<Z *>(p);
  bool ok = z && static_cast<V *>(static_cast<Y *>(z)) == p && dynamic_cast<X *>(p) == z;
  delete p;
  return ok;
}
static_assert(heap());
