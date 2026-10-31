// [expr.delete]/9-10: a class's own operator delete is looked up first
// (unless ::delete); the unsized one is preferred in class scope.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 error "no suitable member 'operator delete' in 'D'"
//@ diag: 11:17 note "member 'operator delete' declared here"
typedef __SIZE_TYPE__ size_t;
struct A { void *operator new(size_t); void operator delete(void *); int x; };
struct B { void *operator new(size_t) = delete; };
struct C { void *operator new(size_t, int); };
struct D { void operator delete(void *, int); };
struct E { void *operator new[](size_t); void operator delete[](void *, size_t); };
struct F : A {};
struct G { void operator delete(void*); void operator delete(void*, size_t); };
void f(A *a, D *d, E *e, F *ff, G *g) {
  delete a;
  ::delete a;
  delete d;
  delete[] e;
  delete ff;
  delete g;
}
