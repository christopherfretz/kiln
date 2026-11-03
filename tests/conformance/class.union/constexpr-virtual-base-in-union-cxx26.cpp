// [class.union]/2, [class.virtual], [expr.dynamic.cast] (P3533R2): an object
// with a virtual base that lives in a union member is found through the
// union's active member, so a virtual base conversion, a virtual call and a
// dynamic_cast use the active member's layout even when another member's
// class has the same base at the same offset (spec/oven.md "base_owner").
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct Base { int b = 1; constexpr virtual int f() const { return b; } };
struct B : virtual Base { int x = 2; constexpr int f() const override { return 20 + x; } };
struct C : B {
  long long y[4] = {3, 0, 0, 0};
  constexpr int f() const override { return 30 + int(y[0]); }
};

union U {
  C c;
  B b;
  constexpr U() : b() {}
  constexpr ~U() { b.~B(); }
};

constexpr int calls() {
  U u;
  Base &r = u.b;
  B *pb = &u.b;
  return r.f() + 100 * pb->f() + 10000 * r.b;
}
static_assert(calls() == 22 + 2200 + 10000);

constexpr int casts() {
  U u;
  Base *p = &u.b;
  return (dynamic_cast<B *>(p) == &u.b) + 2 * (dynamic_cast<C *>(p) == nullptr) +
         4 * (dynamic_cast<void *>(p) == &u.b);
}
static_assert(casts() == 7);

// The other order: the active member is the second one.
union U2 {
  B b;
  C c;
  constexpr U2() : c() {}
  constexpr ~U2() { c.~C(); }
};

constexpr int second() {
  U2 u;
  Base *p = &u.c;
  B &rb = u.c;
  return p->f() + 100 * (dynamic_cast<C *>(p) == &u.c) + 1000 * (static_cast<Base &>(rb).b == 1);
}
static_assert(second() == 33 + 100 + 1000);
