// [basic.lookup.qual.general]/1, [dcl.type.simple]/2: a nested-name-specifier
// component that names nothing (`Und::x y;`) is "use of undeclared identifier
// 'Und'" at that component (in a namespace, "no member named 'Und' in
// namespace 'N'"), and nothing more. As clang (invalid CXXScopeSpec in
// ParseDeclarationSpecifiers / ParseImplicitInt): when a declarator follows,
// the qualified name was the type and the declaration goes on with an invalid
// type, so `y2` is declared and its later use is quiet; otherwise (`;` `=`
// `(`-not-a-declarator) the qualified name is the declarator-id of a
// declaration that declares nothing, so a later `f2` or `k2` is undeclared.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 34:1 error "use of undeclared identifier 'Und'"
//@ diag: 35:12 error "use of undeclared identifier 'Und'"
//@ diag: 36:1 error "use of undeclared identifier 'Und'"
//@ diag: 38:1 error "use of undeclared identifier 'Und'"
//@ diag: 39:1 error "use of undeclared identifier 'Und'"
//@ diag: 40:12 error "use of undeclared identifier 'Und'"
//@ diag: 40:22 error "use of undeclared identifier 'Und'"
//@ diag: 40:40 error "use of undeclared identifier 'g'"
//@ diag: 41:7 error "use of undeclared identifier 'Und'"
//@ diag: 42:1 error "use of undeclared identifier 'Und'"
//@ diag: 44:4 error "no member named 'Und' in namespace 'N'"
//@ diag: 45:12 error "use of undeclared identifier 'Und'"
//@ diag: 46:1 error "use of undeclared identifier 'Und'"
//@ diag: 47:9 error "use of undeclared identifier 'f2'"
//@ diag: 48:1 error "use of undeclared identifier 'Und'"
//@ diag: 49:1 error "use of undeclared identifier 'Und'"
//@ diag: 50:1 error "use of undeclared identifier 'Und'"
//@ diag: 51:1 error "use of undeclared identifier 'Und'"
//@ diag: 52:10 error "use of undeclared identifier 'k2'"
//@ diag: 53:1 error "use of undeclared identifier 'Und'"
//@ diag: 55:5 error "use of undeclared identifier 'Und'"
Und::x y;
void f() { Und::x z; }
Und::x y2;
int i = y2;
Und::x<int> t;
Und::A::B u;
struct M { Und::x m; Und::g(); int c = g; };
const Und::x k = 0;
Und::x *p, q;
namespace N {}
N::Und::x n;
void b() { Und::x z; }
Und::f2();
int a = f2;
Und::h(int);
Und::i(1);
Und::j() {}
Und::k2;
int b2 = k2;
Und::x (y3);
int d = y3;
int Und::w = 1;
