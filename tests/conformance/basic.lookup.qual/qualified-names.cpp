//@ expect: compile-ok
namespace a {
struct S { int m; };
enum E { e1, e2 };
typedef int I;
}
a::I f(a::S s) { return s.m + a::e2; }
int g = ::a::e1;
