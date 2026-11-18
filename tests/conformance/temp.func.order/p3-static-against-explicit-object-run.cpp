// [temp.func.order]/3, CWG2834 (open): a static member function template
// against an explicit object one. kiln follows CWG2834's direction as clang
// 22 does: the explicit object parameter is not compared by deduction (the
// static function takes the other's object parameter), so a more
// specialized second parameter decides, whatever the object parameter's
// type; with only constraints to tell them apart, both compilers report an
// ambiguity for `this auto` and `this Self &&`
// (`p3-implicit-against-explicit-object-ambiguous.cpp`) and differ for the
// other object parameter types (`p3-static-against-explicit-object-kept.cpp`).
// clang 19 reports these calls ambiguous. Found running GCC's C++
// testsuite (WP-1670).
//@ std: c++23
//@ requires: clang>=22
//@ expect: run
//@ stdout: 2222 3333
extern "C" int printf(const char *, ...);
#define STATIC_X(N, X)                                                         \
  struct N {                                                                   \
    static int f(auto) { return 3; }                                           \
    X int f(this auto XQ, auto *) { return 2; }                                \
    static int g(auto *) { return 3; }                                         \
    X int g(this auto XQ, auto) { return 2; }                                  \
  }
#define XQ
STATIC_X(S1, );
#undef XQ
#define XQ &&
STATIC_X(S2, );
struct S3 {
  static int f(auto) { return 3; }
  int f(this S3 &, auto *) { return 2; }
  static int g(auto *) { return 3; }
  int g(this S3 &, auto) { return 2; }
};
struct S4 {
  static int f(auto) { return 3; }
  int f(this S4, auto *) { return 2; }
  static int g(auto *) { return 3; }
  int g(this S4, auto) { return 2; }
};
int main() {
  S1 s1; S2 s2; S3 s3; S4 s4;
  int i = 0;
  printf("%d%d%d%d %d%d%d%d\n", s1.f(&i), s2.f(&i), s3.f(&i), s4.f(&i), s1.g(&i), s2.g(&i),
         s3.g(&i), s4.g(&i));
}
