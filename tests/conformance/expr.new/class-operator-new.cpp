// [expr.new]/12, /19: a class's own operator new is looked up first
// (unless ::new); deleted and non-viable ones are errors.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:10 error "call to deleted function 'operator new'"
//@ diag: 11:18 note "candidate function has been explicitly deleted"
//@ diag: 21:10 error "no matching function for call to 'operator new'"
//@ diag: 12:18 note "candidate function not viable: requires 2 arguments, but 1 was provided"
typedef __SIZE_TYPE__ size_t;
struct A { void *operator new(size_t); void operator delete(void *); int x; };
struct B { void *operator new(size_t) = delete; };
struct C { void *operator new(size_t, int); };
struct D { void operator delete(void *, int); };
struct E { void *operator new[](size_t); void operator delete[](void *, size_t); };
struct F : A {};
struct G { void operator delete(void*); void operator delete(void*, size_t); };
void f() {
  A *p = new A;
  A *q = ::new A;
  B *b = new B;
  C *c = new C;
  C *c2 = new (1) C;
  E *e = new E[3];
  F *ff = new F;
}
