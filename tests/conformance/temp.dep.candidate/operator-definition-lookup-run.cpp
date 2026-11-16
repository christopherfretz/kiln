// [temp.dep.candidate]/1, [over.match.oper]/3, [temp.res.general]/1: the
// non-member candidates of a dependent operator expression are those
// unqualified lookup of the operator name finds at the template's
// definition, plus argument-dependent lookup at instantiation. A better
// operator declared after the definition in a namespace that is not
// associated is not a candidate, for a unary, a binary, a compound
// assignment and a rewritten `!=` from `==` ([over.match.oper]/3.4)
// alike; one found at the definition through a using-declaration is; a
// later declaration in an associated namespace is found by ADL. Found
// running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
namespace N { struct S {}; }
namespace M { struct T {}; }
namespace U { int operator/(N::S, int) { return 4; } }
using U::operator/;
int operator-(N::S, long) { return 10; }
int operator~(const N::S &) { return 100; }
int operator+=(N::S &, long) { return 1000; }
bool operator==(N::S, long) { return true; }
template<class X> int h(X x) { return x - 1; }
template<class X> int c(X x) { return ~x; }
template<class X> int a(X x) { return x += 1; }
template<class X> int q(X x) { return x / 1; }
template<class X> int k(X x) { return x * 1; }
template<class X> bool ne(X x) { return x != 1; }
int operator-(N::S, int) { return 20; }
int operator~(N::S &) { return 200; }
int operator+=(N::S &, int) { return 2000; }
bool operator==(N::S, int) { return false; }
namespace N { int operator*(S, int) { return 3; } }
int main() {
  int v = h(N::S{}) + c(N::S{}) + a(N::S{}) + q(N::S{}) + k(N::S{}) + ne(N::S{});
  return v == 10 + 100 + 1000 + 4 + 3 + 0 ? 0 : 1;
}
