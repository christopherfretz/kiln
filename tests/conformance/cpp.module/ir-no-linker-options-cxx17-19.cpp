// [cpp.module], spec/codegen.md §2: clang 19 emits the empty
// `!llvm.linker.options` only when the language has modules (C++20 and
// later), so `-fclang-compat=19` codegen keys it on the language standard
// (`CodegenOptions::cxx_modules`); it emitted it at every standard
// (WP-769). clang 22 emits none at any standard.
//@ requires: clang<22
//@ expect: ir
//@ std: c++17
int f(int x) { return x + 1; }
struct S { int g() const { return f(1); } };
int h() { return S().g(); }
