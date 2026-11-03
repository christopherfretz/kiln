// [class.virtual]/8, [expr.const] (P3533R2): a virtual call whose final
// overrider has a covariant return type converts the returned pointer or
// reference to the called function's return class; when that class is (in)
// a virtual base of the overrider's, the conversion goes through the returned
// object's most derived object (spec/oven.md "Virtual bases"). Called through
// a base subobject at a non-zero offset and through an intermediate overrider.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct V { int v = 7; constexpr virtual int id() const { return 1; } };
struct P { int pad = 0; constexpr virtual ~P() = default; };
struct D : P, virtual V { int d = 3; constexpr int id() const override { return 2; } };
struct E : D { int e = 5; };

struct A { constexpr virtual V *get() { return nullptr; } constexpr virtual V &ref() = 0; };
struct Q { int q = 9; constexpr virtual int f() const { return q; } };
struct B : Q, A {
  E obj;
  constexpr D *get() override { return &obj; }
  constexpr E &ref() override { return obj; }
};

constexpr int f() {
  B b;
  A *a = &b;          // a base subobject at a non-zero offset
  V *v = a->get();
  V &r = a->ref();
  return (v == static_cast<V *>(&b.obj)) + 2 * (&r == v) + 4 * (v->v == 7) +
         8 * (v->id() == 2) + 16 * (b.get()->d == 3);
}
static_assert(f() == 31);

// Through an intermediate overrider whose return class is itself a
// virtual base of the final one.
struct W : virtual D { };
struct A2 { constexpr virtual V *g() { return nullptr; } };
struct B2 : A2 { W w; constexpr D *g() override { return &w; } };
struct C2 : B2 { constexpr W *g() override { return &w; } };
constexpr int h() {
  C2 c;
  A2 &a = c;
  B2 &bb = c;
  return (a.g() == static_cast<V *>(&c.w)) + 2 * (bb.g() == static_cast<D *>(&c.w)) +
         4 * (a.g()->v == 7);
}
static_assert(h() == 7);
