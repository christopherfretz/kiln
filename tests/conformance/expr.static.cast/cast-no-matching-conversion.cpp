// [expr.static.cast]/4, [expr.type.conv]/2, [expr.cast]/4,
// [dcl.init.general]/16.6.2: a single-operand cast to a class with no
// viable constructor is "no matching conversion for <cast> from 'F' to
// 'T'" with the constructor candidates, "ambiguous conversion for ..." when
// they are ambiguous. For an aggregate the parenthesized aggregate
// initialization (/16.6.2.2) is only a fallback, so its failure is
// reported as that constructor overload failure, not by its element error
// (clang tryDiagnoseOverloadedCast, FK_ParenthesizedListInitFailed); a
// failure in a SFINAE context is silent, and a successful one keeps its
// warnings (WP-316).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 59:9 error "no matching conversion for functional-style cast from 'int' to 'B'"
//@ diag: 52:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 60:9 error "no matching conversion for static_cast from 'int' to 'B'"
//@ diag: 52:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 61:9 error "no matching conversion for C-style cast from 'int' to 'B'"
//@ diag: 52:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'B' for 1st argument"
//@ diag: 52:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 62:9 error "no matching conversion for functional-style cast from 'int' to 'D'"
//@ diag: 54:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 63:9 error "no matching conversion for functional-style cast from 'int' to 'X'"
//@ diag: 56:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X' for 1st argument"
//@ diag: 56:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X' for 1st argument"
//@ diag: 56:12 note "candidate constructor not viable: no known conversion from 'int' to 'int *' for 1st argument"
//@ diag: 64:9 error "no matching conversion for static_cast from 'int' to 'X'"
//@ diag: 56:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X' for 1st argument"
//@ diag: 56:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X' for 1st argument"
//@ diag: 56:12 note "candidate constructor not viable: no known conversion from 'int' to 'int *' for 1st argument"
//@ diag: 65:9 error "ambiguous conversion for functional-style cast from 'double' to 'A'"
//@ diag: 57:12 note "candidate constructor"
//@ diag: 57:21 note "candidate constructor"
//@ diag: 66:11 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 69:40 error "no matching conversion for functional-style cast from 'int' to 'D'"
//@ diag: 70:15 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 54:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 69:52 error "no matching conversion for C-style cast from 'int' to 'D'"
//@ diag: 54:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'D' for 1st argument"
//@ diag: 54:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
struct S { S(int); S(); };
struct B { S arr[2]; };
struct T { T(int); };
struct D { T arr[2]; };
struct F { int x; };
struct X { X(int *); };
struct A { A(long); A(unsigned); };
void g() {
  (void)B(1);
  (void)static_cast<B>(1);
  (void)(B)1;
  (void)D(1);
  (void)X(1);
  (void)static_cast<X>(1);
  (void)A(1.0);
  (void)F(1.5);
  (void)F(1);
}
template <class U> void h(U u) { (void)D(u); (void)(D)u; }
template void h(int);
template <class U> auto k(U u) -> decltype(D(u), 1) { return 1; }
template <class U> int k(...) { return 2; }
int z = k<int>(1);
template <class U> constexpr bool castable = requires(U u) { D(u); };
static_assert(!castable<int>);
static_assert(requires { F(1); });
