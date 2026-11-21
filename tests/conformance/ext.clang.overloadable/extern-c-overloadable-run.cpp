// clang's `overloadable` on C-linkage functions in C++ (clang
// `IsOverload`): two `extern "C"` declarations of one name with different
// parameter types, both `overloadable`, are two functions, mangled as C++
// functions, not conflicting declarations of one C entity. Reduced from
// a module test of WP-1806; kiln reported "conflicting types" (WP-1817).
//@ expect: run
//@ stdout: 101 202 103
extern "C" int printf(const char *, ...);
extern "C" int ov(int x) __attribute__((overloadable));
extern "C" int ov(long x) __attribute__((overloadable));
extern "C" __attribute__((overloadable)) int ov(int x) { return x + 100; }
extern "C" __attribute__((overloadable)) int ov(long x) { return (int)x + 200; }

int main() {
  printf("%d %d %d\n", ov(1), ov(2L), ov(3));
  return 0;
}
