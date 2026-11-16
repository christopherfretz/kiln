// [expr.new]/23: value-initialization `()` and braced lists of an array
// new-expression are valid in every mode (the C++17 sibling
// paren-array-init-cxx17 rejects parenthesized arguments).
//@ std: c++11
//@ expect: compile-ok
struct S { int a; };
struct D { D(); };
int n = 3;
int *p1 = new int[n]();
int *p2 = new int[2]();
S *p3 = new S[2]();
D *p4 = new D[2]();
int *p5 = new int[2]{1, 2};
int *p6 = new int[]{1, 2};
S *p7 = new S[2]{{1}, {2}};
typedef int A2[2];
int *p8 = new A2();
template <class T> T *f() { return new T[2](); }
int *p9 = f<int>();
int *q = new int(1);
S *r = new S{1};
