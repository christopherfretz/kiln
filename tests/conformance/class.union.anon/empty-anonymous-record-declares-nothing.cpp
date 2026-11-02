// [dcl.pre]/5, [class.mem.general]/3: an anonymous union or struct with no
// member declares no name: "declaration does not declare anything"
// [-Wmissing-declarations] at the decl-specifier-seq, after the scope errors,
// in and out of classes. A template's pattern is diagnosed once; its
// instantiation is not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:12 warning "declaration does not declare anything"
//@ diag: 23:24 warning "declaration does not declare anything"
//@ diag: 24:12 warning "declaration does not declare anything"
//@ diag: 24:23 error "anonymous structs and classes must be class members"
//@ diag: 24:23 warning "declaration does not declare anything"
//@ diag: 24:35 warning "declaration does not declare anything"
//@ diag: 25:1 warning "declaration does not declare anything"
//@ diag: 26:30 warning "declaration does not declare anything"
//@ diag: 26:42 warning "declaration does not declare anything"
//@ diag: 29:1 error "anonymous unions at namespace or global scope must be declared 'static'"
//@ diag: 29:1 warning "declaration does not declare anything"
//@ diag: 30:1 warning "declaration does not declare anything"
//@ diag: 31:13 warning "declaration does not declare anything"
//@ diag: 32:30 warning "declaration does not declare anything"
//@ diag: 34:12 warning "declaration does not declare anything"
struct A { struct { }; union { }; int x; };
void f() { union { }; struct { }; static union { }; }
static union { };
template<class T> struct B { struct { }; union { }; T t; };
B<int> b;
struct C { struct { int y; }; union { int : 3; }; };
union { };
static const union { };
namespace { union { }; }
template<class T> void h() { static union { }; }
template void h<int>();
struct D { const struct { }; };
