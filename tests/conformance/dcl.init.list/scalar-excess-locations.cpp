// Too many elements for a scalar are reported at the initialization's location:
// the declarator for a variable, but the '{' of a scalar member's nested list
// and of a default member initializer (clang's InitializationKind locations).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:12 error "excess elements in scalar initializer"
//@ diag: 15:18 error "excess elements in scalar initializer"
//@ diag: 16:5 error "excess elements in scalar initializer"
//@ diag: 17:23 error "excess elements in scalar initializer"
//@ diag: 19:11 error "excess elements in scalar initializer"
//@ diag: 20:8 error "excess elements in scalar initializer"
//@ diag: 21:13 error "excess elements in scalar initializer"
struct H { int a; int b; };
H h1 = {1, {2, 3}};
int arr[2] = {1, {2, 3}};
int x = {1, 2};
void f() { H h2 = {1, {2, 3}}; }
struct M {
  int m = {1, 2};
  int n{1, 2};
  H h = {1, {2, 3}};
};
