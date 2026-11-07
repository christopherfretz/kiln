// [cpp.module], spec/codegen.md §2: in C++20 and later the language has
// modules and clang 19 emits the empty `!llvm.linker.options`, as
// `-fclang-compat=19` codegen does (WP-769; the C++17 sibling has none).
//@ requires: clang<22
//@ expect: ir
//@ std: c++20
int f(int x) { return x + 1; }
struct S { int g() const { return f(1); } };
int h() { return S().g(); }
