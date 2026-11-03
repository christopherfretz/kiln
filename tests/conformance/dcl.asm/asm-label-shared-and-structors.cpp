// [dcl.asm] (GNU asm labels): declarations of different types whose
// labels name one symbol share it, and a label on a constructor,
// destructor or conversion function names the function every variant
// calls (clang: one definition under the label).
//@ expect: run
//@ stdout: 7 1 42 9 3
//@ stdout: dtor 42
#include <cstdio>
int seen = 0;
void set_int(int) asm("setter");
void set_long(long) asm("setter");
void set_long(long v) { seen = (int)v; }
long counter asm("ctr") = 3;
extern int counter_i asm("ctr");
struct S {
  S(int) asm("s_ctor");
  ~S() asm("s_dtor");
  operator int() const asm("s_conv");
  int x;
};
S::S(int v) : x(v) {}
S::~S() { std::printf("dtor %d\n", x); }
S::operator int() const { return x; }
int main() {
  set_long(7);
  int a = seen;
  set_long(1);
  int b = seen;
  int c;
  {
    S s(42);
    c = s;
    std::printf("%d %d %d %d %d\n", a, b, c, seen + 8, counter_i);
  }
}
