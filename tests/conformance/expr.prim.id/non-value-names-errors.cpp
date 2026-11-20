// [expr.prim.id.unqual]/3, [temp.names]/1: an id-expression names a
// value. A class template, alias template or template template
// parameter named without template arguments is clang's "use of ...
// requires template arguments" with the template's note (an alias
// template's at its `using`), and a namespace or namespace alias is
// "unexpected namespace name" (printed as the namespace the alias
// names) (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:19 error "use of class template 'CT' requires template arguments"
//@ diag: 21:26 note "template is declared here"
//@ diag: 29:19 error "use of alias template 'AT' requires template arguments"
//@ diag: 22:19 note "template is declared here"
//@ diag: 30:19 error "unexpected namespace name 'N': expected expression"
//@ diag: 31:19 error "unexpected namespace name 'N': expected expression"
//@ diag: 32:54 error "use of template template parameter 'TT' requires template arguments"
//@ diag: 32:32 note "template is declared here"
// A variable template named with its arguments is a value; `S::M` is
// taken for a type by the parser, as in clang.

template<class T> struct CT {};
template<class T> using AT = int;
namespace N {}
namespace NA = N;
struct S { template<class T> struct M {}; };
template<class T> int vt = 1;

int f1() { return CT; }
int f2() { return AT; }
int f3() { return N; }
int f4() { return NA; }
template<template<class> class TT> int f5() { return TT; }
int f6() { return vt<int>; }
