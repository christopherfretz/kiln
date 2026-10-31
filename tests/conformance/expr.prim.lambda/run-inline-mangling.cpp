// Itanium ABI 5.1.8: lambdas in inline functions and inline variables are
// numbered per context so every translation unit agrees on the closure.
//@ expect: run
//@ stdout: 1 2 3 30
extern "C" int printf(const char*, ...);
inline int f() {
  auto a = [] { return 1; };
  auto b = [] { return 2; };
  auto c = [](int) { return 3; };
  return a() * 100 + b() * 10 + c(0);
}
inline auto iv = [](int x) { return x * 10; };
struct S { static inline auto m = [] { return 3; }; };
int main() { int v = f(); printf("%d %d %d %d\n", v / 100, v / 10 % 10, S::m(), iv(3)); }
