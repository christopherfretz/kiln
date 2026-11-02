// [temp.names]/3.3, [dcl.type.simple]: an undeclared unqualified name
// followed by `<` is a template-name; where a declaration's type is
// expected it is reported as "no template named", the arguments are
// parsed, and the declaration goes on with an invalid type (clang's
// `ParseImplicitInt`). As a call (`f<int>(1)`) it stays an expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:1 error "no template named 'undeclared_t'"
//@ diag: 18:7 error "no template named 'u2'"
//@ diag: 19:8 error "no template named 'u3'"
//@ diag: 20:12 error "no template named 'u4'"
//@ diag: 21:8 error "no template named 'u5'"
//@ diag: 22:12 error "no template named 'u6'"
//@ diag: 23:18 error "use of undeclared identifier 'u7'"
//@ diag: 24:35 error "no template named 'u8' in namespace 'N'"
namespace N { struct S; }
undeclared_t<int> c1;
const u2<int> c2 = 0;
static u3<int> *c3;
struct A { u4<int, 2> m; };
void g(u5<int> p);
void f() { u6<N::S> v; }
int h() { return u7<int>(1); }
int ok = sizeof(int); N::S *p; N::u8<int> c4;
