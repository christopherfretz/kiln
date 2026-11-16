// [expr.static.cast]/7, /11, /12, [expr.cast]/4: no static_cast gives an
// object or function pointer from an operand that is not a pointer (a
// pointer to member, an integer, a floating or enumeration value), nor a
// pointer to member from one that is not a pointer to member. A
// static_cast, and a C-style cast that reinterpret_cast cannot do either,
// reports clang's "cannot cast from type ... to (member) pointer type";
// a reinterpret_cast keeps its own message. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:11 error "cannot cast from type 'int S::*' to pointer type 'void *'"
//@ diag: 28:11 error "cannot cast from type 'void (S::*)()' to pointer type 'void *'"
//@ diag: 29:11 error "cannot cast from type 'int S::*' to pointer type 'void *'"
//@ diag: 30:11 error "reinterpret_cast from 'void (S::*)()' to 'void *' is not allowed"
//@ diag: 32:11 error "cannot cast from type 'int S::*' to pointer type 'void *'"
//@ diag: 33:11 error "cannot cast from type 'int' to pointer type 'void *'"
//@ diag: 34:10 error "cannot cast from type 'double' to pointer type 'int *'"
//@ diag: 35:10 error "cannot cast from type 'E' to pointer type 'int *'"
//@ diag: 36:13 error "cannot cast from type 'int' to member pointer type 'int S::*'"
//@ diag: 37:13 error "cannot cast from type 'int *' to member pointer type 'int S::*'"
//@ diag: 38:10 error "cannot cast from type 'double' to pointer type 'int *'"
//@ diag: 39:13 error "cannot cast from type 'int *' to member pointer type 'int S::*'"
//@ diag: 40:15 error "cannot cast from type 'int S::*' to pointer type 'void (*)()'"
//@ diag: 42:10 error "cannot cast from type 'double' to pointer type 'IP' (aka 'int *')"
struct S { int x; void f(); };
enum E { e0 };
void *a = (void *)&S::x;
void *b = (void *)&S::f;
void *c = static_cast<void *>(&S::x);
void *d = reinterpret_cast<void *>(&S::f);
int S::*pm = &S::x;
void *e = (void *)pm;
void *g = static_cast<void *>(1);
int *h = static_cast<int *>(1.0);
int *i = static_cast<int *>(e0);
int S::*j = static_cast<int S::*>(1);
int S::*k = static_cast<int S::*>((int *)0);
int *l = (int *)1.0;
int S::*m = (int S::*)(int *)0;
void (*n)() = (void (*)())&S::x;
typedef int *IP;
int *o = IP(2.0);
