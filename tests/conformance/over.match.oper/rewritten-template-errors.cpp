// [over.match.oper]/3.4-4, P2468: rewritten and reversed operator
// templates in failed resolutions; an `operator!=` template corresponding
// to an `operator==` template suppresses the reversed candidates.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:21 error "overload resolution selected deleted operator '=='"
//@ diag: 29:25 note "candidate function (with reversed parameter order) [with T = int] has been explicitly deleted"
//@ diag: 29:25 note "candidate function template not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 33:21 error "invalid operands to binary expression ('int' and 'E')"
//@ diag: 29:25 note "candidate function (with reversed parameter order) template not viable: no known conversion from 'E' to 'const D' for 2nd argument"
//@ diag: 29:25 note "candidate function template not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 32:25 note "candidate template ignored: could not match 'T *' against 'E'"
//@ diag: 32:25 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 39:39 error "invalid operands to binary expression ('int' and 'S<int>')"
//@ diag: 29:25 note "candidate function (with reversed parameter order) template not viable: no known conversion from 'S<int>' to 'const D' for 2nd argument"
//@ diag: 29:25 note "candidate function template not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 32:25 note "candidate template ignored: could not match 'T *' against 'S<int>'"
//@ diag: 32:25 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 43:24 error "use of overloaded operator '==' is ambiguous (with operand types 'C' and 'C')"
//@ diag: 41:25 note "candidate function [with T = C]"
//@ diag: 42:25 note "candidate function [with T = C]"
//@ diag: 47:24 error "use of overloaded operator '<' is ambiguous (with operand types 'G' and 'G')"
//@ diag: 45:24 note "candidate function [with T = G]"
//@ diag: 46:25 note "candidate function [with T = G]"
//@ diag: 51:23 error "use of overloaded operator '+' is ambiguous (with operand types 'H' and 'H')"
//@ diag: 49:24 note "candidate function [with T = H]"
//@ diag: 50:24 note "candidate function [with T = H]"
struct D { int v; };
template <class T> bool operator==(const D&, T) = delete;
bool f() { return 1 == D{1}; }
struct E {};
template <class T> bool operator==(E, T*);
bool g() { return 1 == E{}; }
template <class T> struct S {
  T v;
  template <class U> bool operator==(const U& u) const { return v == u; }
  template <class U> bool operator!=(const U& u) const { return v != u; }
};
bool h(S<int> s) { return s == 7 && 7 == s; }
struct C {};
template <class T> bool operator==(C, T);
template <class T> bool operator==(T, C);
bool k(C c) { return c == c; }
struct G {};
template <class T> int operator<=>(G, T);
template <class T> void operator<=>(T, G);
bool m(G e) { return e < e; }
struct H {};
template <class T> int operator+(H, T);
template <class T> int operator+(T, H);
int n(H c) { return c + c; }
