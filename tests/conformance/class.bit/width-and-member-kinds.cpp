// [class.bit]/1, /3, [expr.const]/10: a bit-field width is an integral
// constant expression of integral or unscoped enumeration type (diagnosed at
// the width); a named bit-field has a non-zero width and integral or
// enumeration type (also in a template definition with a dependent width);
// a static data member, typedef or function cannot be a bit-field; `bool`
// never warns about an over-wide width; a member named without an object
// in the width is diagnosed once.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:16 error "named bit-field 'c' has zero width"
//@ diag: 30:36 error "named bit-field 'd' has zero width"
//@ diag: 30:51 error "bit-field 'f' has non-integral type 'float'"
//@ diag: 30:66 error "integral constant expression must have integral or unscoped enumeration type, not 'double'"
//@ diag: 30:75 error "named bit-field 'h' has zero width"
//@ diag: 31:32 error "named bit-field 'z' has zero width"
//@ diag: 32:6 note "in instantiation of template class 'B<0>' requested here"
//@ diag: 33:23 error "static member 's' cannot be a bit-field"
//@ diag: 33:42 error "typedef member 'td' cannot be a bit-field"
//@ diag: 33:56 error "bit-field 'fp' has non-integral type 'int (*)'"
//@ diag: 34:15 error "named bit-field 'k' has zero width"
//@ diag: 37:14 error "named bit-field 'x' has zero width"
//@ diag: 37:26 error "named bit-field 'y' has zero width"
//@ diag: 37:54 error "integral constant expression must have integral or unscoped enumeration type, not 'SE'"
//@ diag: 37:69 error "integral constant expression must have integral or unscoped enumeration type, not 'std::nullptr_t'"
//@ diag: 37:86 error "integral constant expression must have integral or unscoped enumeration type, not 'int *'"
//@ diag: 38:34 error "bit-field 'f' has non-integral type 'float'"
//@ diag: 38:52 error "static member 's' cannot be a bit-field"
//@ diag: 39:27 error "invalid use of non-static data member 'x'"
//@ diag: 41:15 error "bit-field 'fn' has non-integral type 'Fn' (aka 'int ()')"
struct A { int c : 0; int : 0; int d : (0); float f : 3; int g : 2.0; int h : sizeof(int) * 0; };
template<int N> struct B { int z : N; int : N; };
B<0> b0;
struct C { static int s : 3; typedef int td : 2; int (*fp) : 3; };
union U { int k : 0; };
enum E { e0, e1 };
enum class SE { a };
struct D { E x : 0; bool y : 0; bool b9 : 9; int w : SE::a; int v : nullptr; int u : (int*)0; };
template<int N> struct T { float f : N; static int s : N; };
struct M { int x; int f : x; };
typedef int Fn();
struct F { Fn fn : 3; };
