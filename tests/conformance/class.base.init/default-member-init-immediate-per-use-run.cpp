// CWG 2631, [expr.const]/16, [class.base.init]/9, [dcl.init.aggr]/5: an
// immediate invocation in a default member initializer is evaluated at
// each use, in the context of that use: the constructor that does not
// name the member (written, `= default`ed or implicit), or the function
// holding the aggregate initialization. A default member initializer used
// by another one is evaluated in the outermost use's context. Reduced from
// real code.
//@ expect: run
//@ stdout: agg 5 | ctor 1 7 | dflt 1 | impl 1 | tm 5
//@ stdout: nested 4 | nested-ctor 4 | srcloc 39 11 | global 0
#include <source_location>
extern "C" int printf(const char *, ...);
consteval unsigned len(const char *s) { unsigned n = 0; while (s[n]) ++n; return n; }
consteval unsigned sline(std::source_location s) { return s.line(); }
struct A { unsigned n = len(__builtin_FUNCTION()); };
struct C { unsigned n = len(__builtin_FUNCTION()); C() {} C(int) : n(7) {} };
struct D { unsigned n = len(__builtin_FUNCTION()); D() = default; };
struct I { unsigned n = len(__builtin_FUNCTION()); int k = 0; };
template <class T> struct TM { unsigned n = len(__builtin_FUNCTION()); };
struct In { unsigned n = len(__builtin_FUNCTION()); };
struct Out { In in{}; };
struct Out2 { In in{}; Out2(int) {} };
struct L {
  unsigned l = sline(std::source_location::current());
  unsigned f = len(std::source_location::current().function_name());
};
void main2() {
  A a{};
  C c;
  C c7(1);
  D d;
  I i;
  TM<int> t{};
  printf("agg %u | ctor %u %u | dflt %u | impl %u | tm %u\n", a.n, c.n, c7.n, d.n, i.n, t.n);
}
void nest() {
  Out o{};
  Out2 o2(0);
  L l{};
  printf("nested %u | nested-ctor %u | srcloc %u %u", o.in.n, o2.in.n, l.l, l.f);
}
// At namespace scope the use is in no function: the name is empty.
A ga{};
int main() {
  main2();
  nest();
  printf(" | global %u\n", ga.n);
}
