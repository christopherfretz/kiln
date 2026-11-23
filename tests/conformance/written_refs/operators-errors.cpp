// Failed operator resolution (spec/ast.md §18.13.5): an operator
// expression with an operand of class or enumeration type on which
// overload resolution found no viable or no best function has an
// `UNRESOLVED` entry at its token naming nothing (with its close for
// `[]` and `()`); a `->` on a class with no `operator->` too. A failed
// built-in operator (no class or enumeration operand), or one with an
// erroneous operand, has none. A ud-suffix with no literal operator is
// `UNRESOLVED`.
//@ expect: compile-fail
//@ written-refs: operators-errors.refs
//@ diag: 30:5 error "invalid operands to binary expression ('A' and 'A')"
//@ diag: 31:5 error "use of overloaded operator '+' is ambiguous (with operand types 'B' and 'double')"
//@ diag: 32:4 error "type 'A' does not provide a subscript operator"
//@ diag: 33:3 error "type 'A' does not provide a call operator"
//@ diag: 34:5 error "invalid operands to binary expression ('E' and 'int')"
//@ diag: 35:3 error "invalid argument type 'A' to unary expression"
//@ diag: 36:4 error "member reference type 'A' is not a pointer; did you mean to use '.'?"
//@ diag: 36:6 error "no member named 'm' in 'A'"
//@ diag: 37:5 error "invalid operands to binary expression ('int *' and 'int *')"
//@ diag: 38:3 error "use of undeclared identifier 'missing'"
//@ diag: 39:4 error "no matching literal operator for call to 'operator\"\"_none' with argument of type 'unsigned long long' or 'const char *', and no matching literal operator template"
struct A {};
struct B {
  void operator+(int);
  void operator+(long);
};
enum class E { x };

void use(A a, B b, E e, int *p) {
  a + a;
  b + 1.0;
  a[1];
  a(2);
  e + 1;
  -a;
  a->m;
  p + p;
  missing + a;
  1_none;
}
