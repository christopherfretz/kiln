// [dcl.fct] [expr.prim.lambda.closure]/8 A written default calling
// convention (`cdecl`, the default on every target kiln supports) keeps
// the function's type: redeclarations without it, a typedef, overriding,
// deduction, conversions to function pointers (a captureless lambda's
// included), deduced return types and defaulted comparisons all behave as
// without it, and the program links and runs as clang's. Reduced from
// real code. spec/types.md §5.4, WP-1858.
//@ expect: run
//@ stdout: 2 6 6 8 11 7 9 1 5
extern "C" int printf(const char *, ...);
typedef __attribute__((cdecl)) int FC(int);
__attribute__((cdecl)) int c1(int x) { return x + 1; }
FC c2;
int c2(int x) { return x * 2; }
int c3(int);
__attribute__((cdecl)) int c3(int x) { return x + 3; }
template <class F> int call(F *f, int v) { return f(v); }
struct S { virtual __attribute__((cdecl)) int m(int x) { return x - 1; } };
struct D : S { int m(int x) override { return x - 2; } };
template <class T> __attribute__((cdecl)) T tw(T v) { return v + 10; }
__attribute__((cdecl)) auto ar(int x) { return x + 4; }
struct E {
  int v;
  __attribute__((cdecl)) bool operator==(const E &) const = default;
};
int main() {
  int (__attribute__((cdecl)) *p)(int) = c1;
  int (*q)(int) = p;
  D d;
  S &s = d;
  auto l = [](int x) __attribute__((cdecl)) { return x; };
  int (*pl)(int) = l;
  auto g = [](auto x) __attribute__((cdecl)) { return x + 2; };
  int (*pg)(int) = g;
  printf("%d %d %d %d %d %d %d %d %d\n", call(c1, 1), call(&c2, 3), q(5) + c3(0) - 3,
         s.m(10), tw(1), pl(7), pg(7), E{1} == E{1}, ar(1));
}
