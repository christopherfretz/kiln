// [expr.prim.lambda.closure]/5, /8 at C++11 (sibling of
// `p5-lambda-run-before-cxx17-run.cpp`): a call operator or a
// conversion function that is not constexpr changes nothing at run time. A
// lambda in a default member initializer, a default argument, an ordinary
// initializer or a constexpr function's argument is not constant-evaluated
// and stays valid; a pointer from the conversion is initialized
// dynamically. clang 22 and GCC 16.1 accept all of it at C++11 and C++14.
//@ expect: run
//@ std: c++11
//@ stdout: 1 2 3 4 5 6 7

extern "C" int printf(const char *, ...);
struct S { int x = []{ return 1; }(); };
int dflt(int a = []{ return 2; }()) { return a; }
int (*fp)() = []{ return 3; };
auto l = [] { return 4; };
int (*fq)() = l;
constexpr int id(int x) { return x; }
const int c = []{ return 6; }();
auto cl = []() constexpr { return 7; };
int main() {
  S s;
  int k = id([]{ return 5; }());
  printf("%d %d %d %d %d %d %d\n", s.x, dflt(), fp(), fq(), k, c, cl());
}
