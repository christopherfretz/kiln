// Coverage of a stringized macro argument (WP-1083, spec/codegen.md §5.4
// "Instrumentation"): `#c` makes a string token in scratch space, a
// nested expansion whose region clang maps at the `#c` in the macro
// definition; a concatenated string literal ends at its last piece
// (clang `StringLiteral::getEndLoc`).
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
extern "C" int puts(const char *);
#define CHECK(c) if (!(c)) puts("fail: " #c)
#define STR(x) #x
int f(int m) {
  CHECK(m > 0);
  CHECK(m < 100);
  puts(STR(abc) "def");
  return m;
}
