// Without -pedantic, of the flexible array member extensions only a member
// with a flexible array member that is not the last warns
// (-Wgnu-variable-sized-type-not-at-end is on by default; spec/sema.md §5.8).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:14 warning "field 'a' with variable sized type 'A' not at the end of a struct or class is a GNU extension"
struct A { int n; char d[]; };
struct B { char d[]; };
union U { int n; char d[]; };
struct W { A a; int x; };
struct Last { int x; A a; };
struct H { int n; A a[]; };
A g1 = {1, {}};
template <class T> struct X { int n; T d[]; };
X<int> xi;
