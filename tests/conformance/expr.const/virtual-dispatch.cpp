// [class.virtual]/2, [expr.call]/3, [expr.const]/5: virtual calls in
// constant evaluation dispatch on the dynamic type (the final overrider),
// adjust `this` to the overrider's subobject, and apply covariant return
// adjustments; qualified calls and calls on objects of known type are
// static; during construction and destruction the dynamic type is the
// class of the running constructor or destructor ([class.cdtor]/4).
//@ expect: compile-ok
//@ strict-diags
struct A {
  int a = 1;
  constexpr virtual int f() const { return 10 + a; }
  constexpr virtual int g() const { return f() * 2; }
  constexpr virtual ~A() {}
};
struct Pad { int p = 7; constexpr virtual int pad() const { return p; } };
struct B : Pad, A {
  int b = 2;
  constexpr int f() const override { return 20 + b + a; }
};
struct C final : B {
  int c = 3;
  constexpr int f() const override { return 30 + c + B::f(); }
};
constexpr int call(const A &x) { return x.f(); }
static_assert(call(A()) == 11);
static_assert(call(B()) == 23);
static_assert(call(C()) == 56);
constexpr int via_ptr() { C c; A *p = &c; Pad *q = &c; return p->g() + q->pad(); }
static_assert(via_ptr() == 2 * 56 + 7);

// Qualified calls are not virtual ([class.virtual]/16).
struct Q : A { constexpr int f() const override { return 100; } constexpr int base_f() const { return A::f(); } };
constexpr int qualified() { Q q; return q.base_f(); }
static_assert(qualified() == 11);

// A constexpr object's dynamic type is its type; also after thawing.
constexpr C gc;
static_assert(call(gc) == 56);
constexpr const A &gr = gc;
static_assert(gr.f() == 56 && gr.g() == 112);
struct Holder { B b; C cs[2]; };
constexpr Holder gh{};
static_assert(call(gh.b) == 23 && call(gh.cs[1]) == 56);

// Dispatch during construction and destruction ([class.cdtor]/4).
struct Base {
  int seen = 0;
  constexpr virtual int id() const { return 1; }
  constexpr Base() { seen = id(); }
  constexpr virtual ~Base() {}
};
struct Mid : Base {
  int mseen = 0;
  constexpr int id() const override { return 2; }
  constexpr Mid() { mseen = id(); }
};
struct Top : Mid {
  int *log = nullptr;
  constexpr int id() const override { return 3; }
  constexpr Top(int *l) : log(l) {}
  constexpr ~Top() override { *log = *log * 10 + id(); }
};
struct Logged : Base {
  int *log;
  constexpr Logged(int *l) : log(l) {}
  constexpr int id() const override { return 4; }
  constexpr ~Logged() override { *log = id(); }
};
constexpr int ctor_dispatch() { int l = 0; Top t(&l); return t.seen * 100 + t.mseen * 10 + t.id(); }
static_assert(ctor_dispatch() == 123);

// Pointers to virtual member functions dispatch too.
constexpr int mptr() {
  C c;
  int (A::*pf)() const = &A::f;
  const A &r = c;
  B b;
  return (r.*pf)() + (b.*pf)();
}
static_assert(mptr() == 56 + 23);

// Covariant returns are adjusted to the called function's return type.
struct R { int r = 5; constexpr virtual R *self() { return this; } constexpr virtual R &me() { return *this; } };
struct S : Pad, R { constexpr S *self() override { return this; } constexpr S &me() override { return *this; } };
constexpr bool covariant() { S s; R *p = &s; return p->self() == p && &p->me() == p && p->self()->r == 5; }
static_assert(covariant());

// delete through a pointer to a base class with a virtual destructor
// destroys and frees the complete object ([expr.delete]/3).
constexpr int del() { int l = 0; Base *p = new Logged(&l); delete p; return l; }
static_assert(del() == 4);
constexpr int del_null() { A *p = nullptr; delete p; return 1; }
static_assert(del_null() == 1);

// A 3-level hierarchy with an override in the middle only.
struct L1 { constexpr virtual int v() const { return 1; } };
struct L2 : L1 { constexpr int v() const override { return 2; } };
struct L3 : L2 {};
constexpr int levels() { L3 x; const L1 &r = x; int s = 0; for (int i = 0; i < 100; ++i) s += r.v(); return s; }
static_assert(levels() == 200);
