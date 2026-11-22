// [expr.prim.lambda.closure]/7: the call operator's attributes are
// checked before codegen sees them (clang `ProcessDeclAttributes`), so
// one that does not apply is dropped from the IR: `alloc_size` on a
// deduced return type and `nonnull` naming no pointer parameter are not
// emitted, the valid `alloc_size`, `nonnull`, `cold`, `noinline` and
// `section` are. The diagnostics are in call-operator-attribute-checks.cpp.
// C++23, so that the closure objects are not given C++26's erroneous-value
// fill.
// spec/sema.md §4.5, WP-2022.
//@ std: c++23
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: ir
void sink(void *);
int run(int k) {
  auto a = [] (int) __attribute__((alloc_size(2))) -> void * { return nullptr; };
  auto b = [] (int) __attribute__((alloc_size(1))) { return (void *)0; };
  auto c = [] (int *p) __attribute__((nonnull(2))) { return *p; };
  auto d = [] [[gnu::nonnull]] (int n) { return n; };
  auto e = [] () __attribute__((cold, noinline)) { return 1; };
  auto f = [] () __attribute__((section("lsec"))) { return 2; };
  sink(a(4));
  sink(b(4));
  return c(&k) + d(k) + e() + f();
}
