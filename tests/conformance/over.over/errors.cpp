// [over.over]/5: no function matches, or the target type is not a function
// type: the overload set cannot be resolved.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:19 error "address of overloaded function 'f' does not match required type 'void (int)'"
//@ diag: 19:6 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 18:5 note "candidate function has different return type ('void' expected but has 'int')"
//@ diag: 21:27 error "address of overloaded function 'f' does not match required type 'int (int) noexcept'"
//@ diag: 19:6 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 18:5 note "candidate function has different exception specification"
//@ diag: 22:6 error "variable 'p3' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 23:9 error "address of overloaded function 'f' does not match required type 'int'"
//@ diag: 19:6 note "candidate function"
//@ diag: 18:5 note "candidate function"
//@ diag: 28:23 error "address of overloaded function 'm' does not match required type 'void (int)'"
//@ diag: 25:7 note "candidate function has different return type ('void' expected but has 'int')"
//@ diag: 26:7 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
int f(int);
long f(long);
void (*p1)(int) = f;
int (*p2)(int) noexcept = f;
auto p3 = f;
int x = f;
struct A {
  int m(int);
  int m(long);
};
void (A::*pm)(int) = &A::m;
