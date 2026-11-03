// [temp.alias]/2, [temp.dep.type]/7, [temp.res.general]/6: an alias
// template specialization whose aliased type does not depend on the
// arguments (`V<T>` is `void`) is instantiation-dependent but not a
// dependent type, so a template definition using it is checked when it is
// parsed, as for the aliased type itself (spec/sema-templates.md §3.1).
// Each line exercises one check that used to wait for instantiation
// because it asked "instantiation-dependent?" (WP-445 audit).
//@ args: -ferror-limit=0
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:38 error "invalid application of 'sizeof' to an incomplete type 'V<T>' (aka 'void')"
//@ diag: 48:37 error "variable has incomplete type 'V<T>' (aka 'void')"
//@ diag: 49:36 error "allocation of incomplete type 'V<T>' (aka 'void')"
//@ diag: 50:38 error "invalid application of 'alignof' to an incomplete type 'V<T>' (aka 'void')"
//@ diag: 51:32 error "cannot throw object of incomplete type 'V<T>' (aka 'void')"
//@ diag: 52:37 error "field has incomplete type 'V<T>' (aka 'void')"
//@ diag: 54:32 error "base specifier must name a class"
//@ diag: 56:43 error "invalid range expression of type 'I<T>' (aka 'int'); no viable 'begin' function available"
//@ diag: 57:32 error "statement requires expression of integer type ('EE<T>' (aka 'E') invalid)"
//@ diag: 59:37 error "cannot initialize a variable of type 'I<T>' (aka 'int') with an lvalue of type 'const char[2]'"
//@ diag: 60:39 error "cannot initialize return object of type 'I<T>' (aka 'int') with an lvalue of type 'const char[2]'"
//@ diag: 61:41 error "member reference base type 'I<T>' (aka 'int') is not a structure or union"
//@ diag: 62:43 error "invalid operands to binary expression ('EE<T>' (aka 'E') and 'int')"
//@ diag: 63:43 error "no viable conversion from 'EE<T>' (aka 'E') to 'int'"
//@ diag: 64:32 error "cannot delete expression of type 'I<T>' (aka 'int')"
//@ diag: 65:38 error "non-const lvalue reference to type 'I<T>' (aka 'int') cannot bind to a temporary of type 'int'"
//@ diag: 66:61 error "redeclaration of 'ext' with a different type: 'long' vs 'I<T>' (aka 'int')"
//@ diag: 66:44 note "previous declaration is here"
//@ diag: 67:47 error "constexpr variable cannot have non-literal type 'const A<T>' (aka 'const NL')"
//@ diag: 44:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 68:39 error "offsetof requires struct, union, or class type, 'I<T>' (aka 'int') invalid"
//@ diag: 69:46 error "'f' marked 'override' but does not override any member functions"
//@ diag: 78:60 error "cannot initialize a variable of type 'TC<N> *' with an rvalue of type 'int'"
//@ diag: 78:86 error "no viable conversion from 'int' to 'X' (aka 'TC<N>')"
//@ diag: 77:25 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const TC<3> &' for 1st argument"
//@ diag: 77:25 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'TC<3> &&' for 1st argument"
//@ diag: 79:70 error "no viable conversion from 'int' to 'TC<N>'"
//@ diag: 77:25 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const TC<2> &' for 1st argument"
//@ diag: 77:25 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'TC<2> &&' for 1st argument"
template <class T> using V = void;
template <class T> using I = int;
struct E {};
template <class T> using EE = E;
struct NL { NL(); ~NL(); };
template <class T> using A = NL;
// Sema::is_complete (decl.rs): sizeof, alignof, variable, new, throw, field.
template <class T> int f1() { return sizeof(V<T>); }
template <class T> void f2() { V<T> x; }
template <class T> void f3() { new V<T>; }
template <class T> void f4() { (void)alignof(V<T>); }
template <class T> void f5() { throw V<T>(); }
template <class T> struct S6 { V<T> m; };
// A base specifier (class/members.rs): an alias of `int` is no class.
template <class T> struct S7 : I<T> {};
// Range-based for (range_for.rs) and switch (stmt.rs) conditions.
template <class T> void f8() { for (int x : I<T>()) {} }
template <class T> void f9() { switch (EE<T>()) {} }
// Already checked before the audit (kept as a regression guard).
template <class T> void g1() { I<T> x = "s"; }
template <class T> I<T> g2() { return "x"; }
template <class T> void g3() { I<T> x; x.foo(); }
template <class T> void g4() { EE<T> e; e + 1; }
template <class T> void g5(EE<T> e) { int x = e; }
template <class T> void g6() { delete I<T>(); }
template <class T> void g7() { I<T> &r = 1; }
template <class T> void g8() { extern I<T> ext; extern long ext; }
template <class T> void g9() { constexpr A<T> a{}; }
template <class T> void g10() { (void)__builtin_offsetof(I<T>, x); }
template <class T> struct S11 { virtual I<T> f() override; };
// A dependent use still waits: nothing is diagnosed here.
template <class T> using D = T;
template <class T> void h1() { D<T> x = "s"; (void)sizeof(D<T>); }
// A `decltype` of a value-dependent operand is dependent (clang); a
// specialization naming a constant local `N` is only
// instantiation-dependent, so it is checked here.
template <class T> void h2() { decltype(sizeof(T)) x = "s"; }
template <int N> struct TC {};
template <class T> void h3() { constexpr int N = 3; TC<N> *p = 1; using X = TC<N>; X y = 2; }
template <class T> void h4() { [](auto) { constexpr int N = 2; TC<N> q = 1; }; }
