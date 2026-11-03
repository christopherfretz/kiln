// [temp.names]/3, [basic.lookup.qual]/1: a qualified name that names no template
// followed by '<' in a declaration: 'no template named' for an undeclared name or a
// non-template member, 'no type named' without '<'; the error underlines the
// nested-name-specifier (WP-421).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:4 error "no template named 'f' in namespace 'Q'"
//@ diag: 17:4 error "no template named 'g' in namespace 'Q'"
//@ diag: 18:4 error "no template named 'h' in namespace 'Q'"
//@ diag: 20:4 error "no template named 'm' in 'S'"
//@ diag: 22:4 error "no type named 'x' in namespace 'R'"
//@ diag: 23:6 error "no type named 'y' in namespace 'R'"
//@ diag: 24:4 error "no type named 'z' in 'S'"
//@ diag: 25:7 error "no type named 'w' in 'R::T'"
namespace Q { int f; template <class T> struct X {}; void h(); }
Q::f<int> z;
Q::g<int> z2;
Q::h<int> z3;
struct S { int m; };
S::m<int> z4;
namespace R { int v; struct T {}; }
R::x a;
::R::y b;
S::z c;
R::T::w d;
Q::X<int> ok;
