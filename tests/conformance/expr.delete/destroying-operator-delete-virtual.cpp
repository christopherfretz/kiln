// [expr.delete]/7, /10, [class.dtor]/13: deleting through a pointer to a
// base with a virtual destructor calls the deallocation function found in
// the most-derived class's scope; when that is a destroying `operator
// delete` it receives the object converted to its first parameter type
// and the size of the most-derived class, and no destructor runs unless
// it runs one itself. A derived class's own usual `operator delete` hides
// an inherited destroying one.
//@ expect: run
//@ stdout: B::destroy at-base size-of-D
//@ stdout: A::destroy 3
//@ stdout: ~A
//@ stdout: ~U
//@ stdout: ~A
//@ stdout: U::delete
extern "C" int printf(const char *, ...);
extern "C" void free(void *);
extern "C" void *malloc(__SIZE_TYPE__);
namespace std {
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
struct X { virtual ~X() { printf("~X\n"); } long x = 0; };
struct B {
  virtual ~B() { printf("~B\n"); }
  void *operator new(__SIZE_TYPE__ n) { return malloc(n); }
  void operator delete(B *p, std::destroying_delete_t, __SIZE_TYPE__ n);
};
struct D : X, B { ~D() { printf("~D\n"); } int d[5]; };
void *seen;
void B::operator delete(B *p, std::destroying_delete_t, __SIZE_TYPE__ n) {
  printf("B::destroy %s %s\n", (void *)p == seen ? "at-base" : "elsewhere",
         n == sizeof(D) ? "size-of-D" : "other-size");
  free(dynamic_cast<void *>(p));
}
struct A {
  int v = 3;
  virtual ~A() { printf("~A\n"); }
  void *operator new(__SIZE_TYPE__ n) { return malloc(n); }
  void operator delete(A *p, std::destroying_delete_t) {
    printf("A::destroy %d\n", p->v);
    p->~A();
    free(p);
  }
};
struct U : A {
  ~U() { printf("~U\n"); }
  void operator delete(void *p) { printf("U::delete\n"); free(p); }
};
int main() {
  D *d = new D;
  B *b = d;
  seen = b;
  delete b;
  A *a = new A;
  delete a;
  A *u = new U;
  delete u;
  return 0;
}
