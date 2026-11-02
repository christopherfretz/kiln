// [class.mi]/4, [class.base.init]/13, [class.copy.ctor]/14,
// [class.copy.assign]/12, [expr.new], [expr.delete]/3, [class.dtor]/13
// (C++26, P3533R2): virtual bases of virtual bases during constant
// evaluation and at run time, with the same results: a virtual base shared
// by a virtual and a non-virtual path, objects of such classes as array
// elements of a member, a heap object deleted through a pointer to its
// virtual base (the virtual destructor runs the complete-object
// destructor), and the implicit copy constructor and copy assignment
// (each copies the shared virtual base once). The values were checked
// against clang 19's run time with `constexpr` removed (no clang
// implements P3533).
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: run
//@ strict-diags
//@ stdout: 135 2111129 217878 3
//@ stdout: 135 2111129 217878 3
extern "C" int printf(const char *, ...);

struct V {
  int v = 1;
  constexpr virtual int f() const { return v; }
  constexpr virtual ~V() = default;
};
struct W : virtual V { int w = 2; constexpr int f() const override { return 20 + v; } };
struct X : virtual V { int x = 3; };
struct Y : virtual W, virtual X { int y = 4; };
struct Z : virtual Y, virtual X {
  int z = 5;
  int *dtors = nullptr;
  constexpr ~Z() override {
    if (dtors)
      ++*dtors;
  }
};
struct M { int pad; Y y; Z zs[2]; };

constexpr int shared(int one) {
  Z z;
  V &v1 = static_cast<W &>(z);
  V &v2 = static_cast<X &>(z);
  int r = (&v1 == &v2) * one;
  r = r * 10 + z.f() / 10;
  r = r * 10 + z.v + z.w + z.x + z.y + z.z;
  return r;
}

constexpr int in_member(int nine) {
  M m{};
  m.zs[1].v = nine;
  V *p = &m.zs[1];
  int r = p->f();
  r += (dynamic_cast<void *>(p) == static_cast<void *>(&m.zs[1])) * 100;
  r += (dynamic_cast<Y *>(p) == static_cast<Y *>(&m.zs[1])) * 1000;
  r += (dynamic_cast<W *>(static_cast<X *>(&m.y)) == static_cast<W *>(&m.y)) * 10000;
  return r + m.y.f() * 100000;
}

constexpr int heap_and_copy(int seven) {
  V *p = new Z;
  int r = p->f();
  delete p;
  Z a;
  a.v = seven;
  a.z = 8;
  Z b(a);
  Z c;
  c = a;
  return r * 10000 + b.v * 1000 + b.z * 100 + c.v * 10 + c.z;
}

constexpr int dtor_count() {
  int n = 0;
  V *p = new Z;
  dynamic_cast<Z *>(dynamic_cast<X *>(dynamic_cast<W *>(p)))->dtors = &n;
  delete p;
  {
    Z z;
    z.dtors = &n;
    Z copy(z);
  }
  return n;
}

constexpr int ct1 = shared(1), ct2 = in_member(9), ct3 = heap_and_copy(7), ct4 = dtor_count();
static_assert(ct1 == 135 && ct2 == 2111129 && ct3 == 217878 && ct4 == 3);

int main(int argc, char **) {
  printf("%d %d %d %d\n", ct1, ct2, ct3, ct4);
  printf("%d %d %d %d\n", shared(argc), in_member(argc + 8), heap_and_copy(argc + 6), dtor_count());
}
