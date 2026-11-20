// [cpp.pragma]/1: #pragma clang section and templates: an instantiation takes the sections
// of its template's definition (a static data member's out-of-line definition,
// a static local, a member function), wherever it is instantiated; an
// in-class member function definition takes the section current at its
// declarator; implicit globals (vtables, type info) get none. WP-1744.
//@ expect: ir
//@ std: c++20
template <class T> struct X { static int m; static inline int n = 2; void f() {} T g(); };
#pragma clang section bss="B6" data="D6" text="T6"
template <class T> int X<T>::m = 1;
template <class T> T X<T>::g() { static T s; return s + m + n; }
template <class T> int ft() { static int q = 3; return q; }
struct Y {
#pragma clang section text="IN"
  int a() { return 1; }
#pragma clang section text=""
  int b() { return 2; }
  virtual ~Y() {}
};
#pragma clang section text="T7"

int use() { X<int> x; x.f(); Y y; return x.g() + ft<int>() + y.a() + y.b(); }
static int sv;
int *psv = &sv;
static const int sc[] = {1, 2};
const int *psc = sc;
