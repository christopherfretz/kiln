//@ args: -std=c++26
// Class visibility (spec/codegen.md "Symbol visibility"; clang
// getLVForClassMember): a class's `visibility` attribute reaches its
// member functions, static data members, vtable, VTT, construction
// vtables, typeinfo and typeinfo name, and thunks; a member's own
// attribute overrides it. A derived class of a hidden base is not
// itself hidden, but a construction vtable follows the most-derived
// class.
struct __attribute__((visibility("hidden"))) Base {
  virtual ~Base();
  virtual int f();
  __attribute__((visibility("default"))) int exported();
  static int s;
};
Base::~Base() {}
int Base::f() { return 1; }
int Base::exported() { return 2; }
int Base::s = 3;

struct Other {
  virtual int g();
};
int Other::g() { return 4; }

struct __attribute__((visibility("hidden"))) Multi : Other, Base {
  int f() override;
  int g() override;
};
int Multi::f() { return 5; }
int Multi::g() { return 6; }

struct A {
  virtual void a();
};
struct B : virtual A {
  virtual void b();
};
struct __attribute__((visibility("hidden"))) C : B {
  virtual void c();
};
void C::c() {}

struct __attribute__((visibility("protected"))) P {
  virtual void p();
};
void P::p() {}

int use() {
  Multi m;
  C c;
  try {
    throw m;
  } catch (Base &) {
  }
  return m.f() + Base::s;
}
