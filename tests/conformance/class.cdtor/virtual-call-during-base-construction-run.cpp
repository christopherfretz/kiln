// [class.cdtor]/4: during the construction of a base subobject the dynamic
// type is that base: a virtual call from the base's constructor body, from
// a member initializer's argument (after the base's own vptrs are set,
// [class.base.init]/13), and through `this` converted to a virtual base all
// pick the base's final overrider, also when the class under construction
// is itself a base of a class with a virtual base (construction vtables).
// Found running GCC's C++ testsuite. Matches clang 22 and GCC 16.1.
//@ expect: run
//@ strict-diags
//@ stdout: V:V arg:A A:A B:B C:C|V:V arg:A A:A B:B C:C|
extern "C" int printf(const char*, ...);
struct V {
  virtual const char* n() const { return "V"; }
  V() { printf("V:%s ", n()); }
};
struct A : virtual V {
  const char* n() const override { return "A"; }
  int x;
  static int pr(const V* v) { printf("arg:%s ", v->n()); return 0; }
  A() : x(pr(this)) { printf("A:%s ", n()); }
};
struct B : virtual V {
  const char* n() const override { return "B"; }
  B() { printf("B:%s ", static_cast<V*>(this)->n()); }
};
struct C : A, B {
  const char* n() const override { return "C"; }
  C() { printf("C:%s|", n()); }
};
struct D : C {
  const char* n() const override { return "D"; }
};
int main() {
  C c;
  D d;
  printf("\n");
}
