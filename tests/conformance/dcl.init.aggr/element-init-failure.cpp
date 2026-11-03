// [dcl.init.aggr]/5.2, [dcl.init.aggr]/12: an array element (or member)
// copy-initialized from a void call cannot be initialized; once one
// element failed, or a nested list had excess clauses, no "excess
// elements" error follows (clang's InitListChecker hadError), and the
// failed initialization is not analyzed as a full-expression nor usable
// in a constant expression. The excess error is at the start of the
// first excess clause. Narrowing ([dcl.init.list]/3.9) is not a failure.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 28:12 error "cannot initialize an array element of type 'int' with an rvalue of type 'void'"
//@ diag: 29:13 error "cannot initialize an array element of type 'int' with an rvalue of type 'void'"
//@ diag: 30:16 error "excess elements in array initializer"
//@ diag: 32:8 error "cannot initialize a member subobject of type 'int' with an rvalue of type 'void'"
//@ diag: 33:26 error "cannot initialize a member subobject of type 'int' with an rvalue of type 'void'"
//@ diag: 34:17 error "cannot initialize an array element of type 'int' with an rvalue of type 'void'"
//@ diag: 35:23 error "excess elements in array initializer"
//@ diag: 36:13 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 36:13 note "insert an explicit cast to silence this issue"
//@ diag: 36:18 error "excess elements in array initializer"
//@ diag: 37:16 error "excess elements in array initializer"
//@ diag: 38:16 error "excess elements in array initializer"
//@ diag: 39:13 error "cannot initialize an array element of type 'int' with an rvalue of type 'void'"
//@ diag: 39:22 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 39:22 note "insert an explicit cast to silence this issue"
//@ diag: 40:26 error "excess elements in array initializer"
void g(int, int);
int a[] = {g(1, 2), 2};
int b[2] = {g(1, 2), 2, 3};
int c[1] = {1, g(1, 2)};
struct S { int x; int y; };
S s = {g(1, 2), 2};
struct T { int x; } t = {g(1, 2), 2};
int d[2][1] = {{g(1, 2)}, {1}, {2}};
int e[1][2] = {{1, 2, 3}, 4};
int f[1] = {1.5, 2};
int h[1] = {1, (g)(1, 2)};
int k[1] = {1, 1 + 2};
int m[2] = {g(1, 2), 1.5};
constexpr int n[1] = {1, 2};
static_assert(n[0] == 2);
