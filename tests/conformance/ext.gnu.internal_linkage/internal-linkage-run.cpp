// clang `internal_linkage` (AttrDocs "internal_linkage"; WP-807): a
// function, variable, class (its members included), function or variable
// template, or class template with it has internal linkage, as if `static`
// or in an unnamed namespace: it mangles with `L` at namespace scope and is
// emitted `internal`, and still behaves as an ordinary entity in its TU.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8 9 10
extern "C" int printf(const char*, ...);

__attribute__((internal_linkage)) int f() { return 1; }
[[clang::internal_linkage]] int v = 2;
struct S {
  __attribute__((internal_linkage)) int m() { return 3; }
  __attribute__((internal_linkage)) static int sv;
};
int S::sv = 4;
class __attribute__((internal_linkage)) C {
public:
  virtual int g() { return 5; }
  virtual ~C() {}
};
struct D : C { int g() override { return 6; } };
template <class T> __attribute__((internal_linkage)) T tf() { return T(7); }
template <class T> __attribute__((internal_linkage)) T vt = T(8);
template <class T> struct __attribute__((internal_linkage)) CT { static T sv; };
template <class T> T CT<T>::sv = T(9);
namespace N { __attribute__((internal_linkage)) int nv = 10; }

int main() {
  S s;
  D d;
  C *c = &d;
  C base;
  int (*fp)() = f;
  printf("%d %d %d %d %d %d %d %d %d %d\n", fp(), v, s.m(), S::sv, base.g(), c->g(),
         tf<int>(), vt<int>, CT<int>::sv, N::nv);
}
