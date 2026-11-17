// [class.union.anon]/1, [dcl.init.general]/7: the unnamed object of an
// anonymous union at namespace or block scope is default-initialized, so
// a default member initializer of one of its members, at any level of
// nested anonymous unions, initializes it, at run time and in a constant
// evaluation. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ stdout: 3 4 5 6
extern "C" int printf(const char*, ...);
static union { int a = 3; long b; };
static union { union { int c{4}; char d; }; long e; };
constexpr int local() {
  union { union { int p{6}; char q; }; long r; };
  return p;
}
static_assert(local() == 6);
int main() {
  union { int x = 5; long y; };
  union { union { int p{6}; char q; }; long r; };
  printf("%d %d %d %d\n", a, c, x, p);
}
