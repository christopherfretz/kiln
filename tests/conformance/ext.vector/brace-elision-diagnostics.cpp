// Brace elision of vector members ([dcl.init.aggr]/16; clang 19
// CheckVectorType under CheckImplicitInitList): a vector item inside an
// elided vector sub-list is "cannot initialize a vector element", elided
// elements are checked for narrowing, and items past the aggregate are
// excess (WP-173).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 24:16 error "cannot initialize a vector element of type 'int' with an lvalue of type 'g4i' (vector of 4 'int' values)"
//@ diag: 25:14 error "cannot initialize a vector element of type 'int' with an lvalue of type 'const g4i' (vector of 4 'int' values)"
//@ diag: 26:25 error "excess elements in struct initializer"
//@ diag: 27:22 error "type 'float' cannot be narrowed to 'int' in initializer list"
//@ diag: 27:22 note "insert an explicit cast to silence this issue"
//@ diag: 28:22 error "non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list"
//@ diag: 28:22 note "insert an explicit cast to silence this issue"
//@ diag: 29:32 error "cannot initialize a vector element of type 'int' with an lvalue of type 'g4i' (vector of 4 'int' values)"
//@ diag: 30:22 error "cannot initialize a vector element of type 'int' with an lvalue of type 'g4i' (vector of 4 'int' values)"
//@ diag: 31:14 error "cannot initialize a vector element of type 'int' with an lvalue of type 'g4i' (vector of 4 'int' values)"
typedef int g4i __attribute__((vector_size(16)));
typedef float g2f __attribute__((vector_size(8)));
struct Q { g4i v; int z; };
struct P { g2f p; int z; };
void f(g4i a, const g4i &r, int i, float x) {
  Q d = {1, 2, a, 5};
  Q d2 = {1, r};
  Q h = {1, 2, 3, 4, 5, 6};
  P p = {1.5f, 2.5f, x};
  Q n = {1, i, 3, 4, (long)i};
  g4i arr[2] = {1, 2, 3, 4, 5, a};
  g4i ok[2] = {1, 2, a};
  Q w = {{1, a}, 2};
}
