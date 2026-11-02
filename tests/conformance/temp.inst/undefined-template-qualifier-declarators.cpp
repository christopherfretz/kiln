// [temp.inst]/2, [dcl.dcl]/1: a decl-specifier `X<int>::t` naming a member of an undefined
// template's specialization is "implicit instantiation of undefined template" once per
// declaration, however many declarators follow (`f, &g`); clang prints the group twice at
// namespace and class scope (its decl-specifier parse and `ParseImplicitInt` each complete the
// qualifier, spec/diag.md §7.4.1 `dup(clang)`), once at block scope.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:1 error "implicit instantiation of undefined template 'X<int>'" dup(clang)
//@ diag: 16:27 note "template is declared here"
//@ diag: 18:8 error "implicit instantiation of undefined template 'X<int>'" dup(clang)
//@ diag: 16:27 note "template is declared here"
//@ diag: 19:12 error "implicit instantiation of undefined template 'X<int>'" dup(clang)
//@ diag: 16:27 note "template is declared here"
//@ diag: 20:12 error "implicit instantiation of undefined template 'X<int>'"
//@ diag: 16:27 note "template is declared here"
template <class T> struct X;
X<int>::t f, &g;
static X<int>::t s1, *s2, s3;
struct S { X<int>::t m, &n; };
void h() { X<int>::t a, &b; }
