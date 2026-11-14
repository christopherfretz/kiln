// C23 6.7.11p11, 6.5.17.2p1: an atomic object's initializer that
// violates the simple-assignment constraints of its non-atomic type is
// diagnosed with the C assignment wording, naming the atomic type (clang
// `DiagnoseAssignmentResult`).
//@ args: -std=gnu17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:18 error "initializing '_Atomic(int *)' with an expression of incompatible type 'double'"
//@ diag: 26:18 error "incompatible integer to pointer conversion initializing '_Atomic(int *)' with an expression of type 'int'"
//@ diag: 27:21 error "initializing '_Atomic(struct S)' with an expression of incompatible type 'struct T'"
//@ diag: 28:18 error "initializing '_Atomic(int *)' with an expression of incompatible type 'double'"
//@ diag: 29:16 error "incompatible pointer to integer conversion initializing '_Atomic(int)' with an expression of type 'int *'"
//@ diag: 30:5 error "assigning to '_Atomic(int *)' from incompatible type 'double'"
//@ diag: 31:5 error "passing 'double' to parameter of incompatible type '_Atomic(int *)'"
//@ diag: 23:23 note "passing argument to parameter 'p' here"
//@ diag: 34:18 error "initializing 'int' with an expression of incompatible type '_Atomic(struct S)'"
//@ diag: 35:7 error "initializing 'int' with an expression of incompatible type '_Atomic(struct S)'"
//@ diag: 36:7 error "assigning to 'struct T' from incompatible type '_Atomic(struct S)'"
struct S { int a, b; };
struct T { int a; };
struct s { _Atomic(int *) p; };
struct T gt; int *ip; double d;
void g(_Atomic(int *) p);
void f(void) {
  _Atomic(int *) a = d;
  _Atomic(int *) e = 5;
  _Atomic(struct S) as = gt;
  struct s m = { d };
  _Atomic(int) ai = ip;
  a = d;
  g(d);
}
void k(_Atomic(struct S) as, struct T *pt) {
  struct S l = { as };
  int i = as;
  *pt = as;
}
