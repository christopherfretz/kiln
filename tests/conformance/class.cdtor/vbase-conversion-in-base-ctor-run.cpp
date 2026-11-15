// [class.cdtor]/3-6: while the base-subobject constructor of C runs for a more-derived object,
// converting `this` to the virtual base V must read V's offset from the construction vtable
// (through the VTT), because V sits at a different offset in C, in E (C a non-virtual base) and
// in F (E a virtual base). That holds in a base initializer (`Sink(this)`, so the vtable
// pointers are stored before it, as clang's `BaseInitializerUsesThis`), a default member
// initializer, the body and a lambda; a virtual call, `dynamic_cast` and `typeid` through the
// V pointer see C during construction and destruction. Found running GCC's C++ testsuite.
//@ expect: run
//@ args: -Wno-inaccessible-base
//@ stdout: ok
#include <cstdio>
#include <typeinfo>
int fails = 0;
void check(bool ok, int line) { if (!ok) { std::printf("line %d\n", line); ++fails; } }
#define CHECK(x) check(x, __LINE__)
struct V { int v = 7; virtual const char* who() { return "V"; } virtual ~V() {} };
struct Sink { V* got; Sink(V* p) : got(p) {} };
V* seen_lambda;
// C has V at one offset as a complete object and at another inside E.
struct C : Sink, virtual V {
  int pad[3] = {1, 2, 3};
  V* dmi = this;                     // default member initializer
  V* body;
  V* viaf;
  const char* who_ctor;
  const char* who_dtor;
  bool dc_ok, ti_ok;
  V* self() { return this; }
  C() : Sink(this), viaf(self()) {
    body = this;
    [this] { seen_lambda = this; }();
    V* vp = this;
    who_ctor = vp->who();            // [class.cdtor]/4: C's override
    dc_ok = dynamic_cast<C*>(vp) == this && dynamic_cast<void*>(vp) == (void*)this;
    ti_ok = typeid(*vp) == typeid(C);
  }
  ~C() {
    V* vp = this;
    who_dtor = vp->who();
    dtor_dc = dynamic_cast<C*>(vp) == this;
    dtor_ti = typeid(*vp) == typeid(C);
  }
  const char* who() override { return "C"; }
  static bool dtor_dc, dtor_ti;
};
bool C::dtor_dc, C::dtor_ti;
struct Pad { long long q[5] = {}; };
struct E : Pad, C { const char* who() override { return "E"; } };
struct F : Pad, virtual E { int f = 0; const char* who() override { return "F"; } };
template <class T> void run(const char* name) {
  T* t = new T;
  C* c = t;
  V* v = t;
  CHECK(c->got == v);
  CHECK(c->dmi == v);
  CHECK(c->body == v);
  CHECK(c->viaf == v);
  CHECK(seen_lambda == v);
  CHECK(c->who_ctor[0] == 'C');
  CHECK(c->dc_ok);
  CHECK(c->ti_ok);
  delete t;
  CHECK(C::dtor_dc);
  CHECK(C::dtor_ti);
  (void)name;
}
int main() {
  run<C>("C");
  run<E>("E");
  run<F>("F");
  if (!fails) std::puts("ok");
  return fails;
}
