// [temp.names]/1, [dcl.meaning.general]/2: a template-id naming a
// non-template class without `template<>`, and a class-head or
// elaborated name qualified by an undeclared namespace.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:8 error "explicit specialization of non-template struct 'NT'"
//@ diag: 15:11 error "explicit specialization of non-template struct 'P'"
//@ diag: 17:8 error "use of undeclared identifier 'NS5'"
//@ diag: 18:8 error "use of undeclared identifier 'NS6'"
//@ diag: 19:1 error "unknown type name 'Y'"
//@ diag: 21:8 error "use of undeclared identifier 'NS7'"
struct NT;
struct NT<int> {};
namespace N { struct P; }
struct N::P<int> *pp;
template<class T> struct Z;
struct NS5::Z { int x; };
struct NS6::Y { int y; };
Y *p;
int Z2;
struct NS7::Z2 *q;
