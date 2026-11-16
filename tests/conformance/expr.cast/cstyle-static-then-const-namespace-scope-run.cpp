// [expr.cast]/4.3, [basic.start.static]: a C-style cast from a pointer to
// const derived to a pointer to a base at a non-zero offset (the derived
// class has a vptr, the base none) is a static_cast followed by a
// const_cast, so it adds the base offset in the initializer of a global,
// dynamic (from a non-constant pointer) or constant (from the address of a
// global), and in a function. No temporary is involved. Found running GCC's
// C++ testsuite. Matches clang 22 and GCC 16.1.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 | 1 2 | 1 2 | 3
extern "C" int printf(const char*, ...);
struct Base {
  int a;
  int b;
};
struct Derived : Base {
  Derived() { a = 1; b = 2; }
  virtual ~Derived() {}
  int c = 3;
};
Derived obj;
const Derived* objPtr = &obj;
Base* dyn = (Base*)objPtr;
Base* cst = (Base*)(const Derived*)&obj;
static void show(const Base* p) { printf("%d %d | ", p->a, p->b); }
int main() {
  show(dyn);
  show(cst);
  Base* loc = (Base*)objPtr;
  show(loc);
  printf("%d\n", ((Derived*)(const Base*)loc)->c);
}
