// `-fshort-enums`: the layout of enumerations without a fixed underlying
// type at run time, in a structure, and their values round-trip (found
// running GCC's C++ testsuite). WP-1461.
//@ expect: run
//@ args: -fshort-enums
//@ stdout: 4 4 8 1 1 2
//@ stdout: -1 2147483648 -128 127
extern "C" int printf(const char *, ...);
enum A { a1 = 0x7fffffff };
enum B { b1 = 0x80000000 };
enum C { c1 = -1, c2 = 0x80000000 };
enum D { d1 = -128, d2 = 127 };
enum E { e1 = -128, e2 = -128 };
struct P {
  D d;
  char c;
};
int main() {
  C c = c2;
  D d = d1;
  volatile D e = d2;
  printf("%d %d %d %d %d %d\n", (int)sizeof(A), (int)sizeof(B), (int)sizeof(C),
         (int)sizeof(D), (int)sizeof(E), (int)sizeof(P));
  printf("%ld %ld %d %d\n", (long)c1, (long)c, (int)d, (int)e);
}
