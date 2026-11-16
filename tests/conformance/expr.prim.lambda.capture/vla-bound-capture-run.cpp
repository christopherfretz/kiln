//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ stdout: 32 23 12 12 12 41 12 12 16 5
// [expr.prim.lambda.capture] with variable length arrays (GNU extension):
// a lambda that captures a variable length array by reference, or uses a
// typedef of one, gets each array bound as a hidden closure member (clang
// `captureVariablyModifiedType`), so `sizeof` and indexing in the body use
// the size the array was created with, not the bound expression evaluated
// again; `sizeof` of a variable length array operand is potentially
// evaluated, so the array it names is captured too. A variable length array declared in a lambda body works as in a
// function body. Found running GCC's C++ testsuite.
extern "C" int printf(const char *, ...);

int by_default(int n) {
  int a[n];
  for (int i = 0; i < n; ++i) a[i] = i * 10;
  return [&] { return a[n - 1] + (int)sizeof a; }();
}
int named_then_changed(int n) {
  int a[n];
  a[0] = 7;
  auto f = [&a] { return a[0] + (int)sizeof(a); };
  n = 99;
  return f();
}
int declared_in_body(int n) {
  return [&] { int b[n]; return (int)sizeof b; }();
}
int declared_in_body_copy(int n) {
  return [n] { int b[n]; return (int)sizeof b; }();
}
int declared_in_body_param(int n) {
  return [](int m) { int b[m]; return (int)sizeof b; }(n);
}
int two_dims(int n) {
  int a[n][n + 1];
  a[1][1] = 5;
  return [&] { return a[1][1] + (int)sizeof a[0] + (int)sizeof a; }();
}
int typedef_only(int n) {
  typedef int A[n];
  n = 1;
  return [&] { return (int)sizeof(A); }();
}
int nested(int n) {
  int a[n];
  return [&] { return [&] { return (int)sizeof a; }(); }();
}
int sizeof_operand_only(int n) {
  int a[n];
  n = 50;
  return [&] { return (int)sizeof a + (int)sizeof(decltype(a)); }();
}
int generic(int n) {
  int a[n];
  a[2] = 5;
  return [&](auto i) { return a[i]; }(2);
}

int main() {
  printf("%d %d %d %d %d %d %d %d %d %d\n", by_default(3), named_then_changed(4),
         declared_in_body(3), declared_in_body_copy(3), declared_in_body_param(3),
         two_dims(2), typedef_only(3), nested(3),
         sizeof_operand_only(2), generic(3));
}
