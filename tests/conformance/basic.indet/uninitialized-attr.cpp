//@ expect: compile-ok
//@ verify
// [[clang::uninitialized]] (spec/codegen.md §7y) opts one local variable
// out of -ftrivial-auto-var-init and of C++26's erroneous-value fill. It
// applies only to local variables; elsewhere it is ignored with a
// warning, as clang does.
[[clang::uninitialized]] int global; // expected-warning {{attribute only applies to local variables}}
void f() {
  [[clang::uninitialized]] int x;
  __attribute__((uninitialized)) int y;
  [[clang::uninitialized]] static int s; // expected-warning {{attribute only applies to local variables}}
  (void)x;
  (void)y;
  (void)s;
}
struct S {
  [[clang::uninitialized]] int m; // expected-warning {{attribute only applies to local variables}}
};
