// [over.match.oper]/3.4, [class.compare.default]: rewritten and reversed comparisons, and
// the rewritten-!= non-bool warning (prvalue left operands: no 'use =' fix-it note).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:7 warning "equality comparison result unused"
//@ diag: 24:25 warning "equality comparison result unused"
//@ diag: 26:7 warning "equality comparison result unused"
//@ diag: 27:5 warning "ISO C++20 requires return type of selected 'operator==' function for rewritten '!=' comparison to be 'bool', not 'int'"
//@ diag: 22:16 note "declared here"
//@ diag: 24:17 warning "expression result unused"
//@ diag: 24:33 warning "expression result unused"
//@ diag: 24:41 warning "expression result unused"
//@ diag: 24:49 warning "expression result unused"
//@ diag: 25:5 warning "expression result unused"
//@ diag: 25:12 warning "expression result unused"
//@ diag: 27:5 warning "expression result unused"
struct A { int v; bool operator==(const A&) const; };
struct B { int v; };
bool operator==(B, int);
struct C { int operator<=>(const C&) const; };
struct D { bool operator==(D); };
struct E { int operator==(const E&) const; };
void f(A a, B b, C c, D d, E e) {
  A{} == a; A{} != a; b == 1; 1 == b; b != 1; 1 != b;
  c < c; c >= c;
  D{} == d;
  e != e;
}
