// [temp.alias]/2 with [dcl.type.general]/3: an alias template's type-id may
// not define a class or enumeration (a non-template alias-declaration may;
// see dcl.type/type-definition-allowed-contexts-run.cpp). The defined class
// does not take the template head, so its members are ordinary members.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:29 error "'(unnamed struct at "
//@ diag: 13:36 error "'S2' cannot be defined in a type alias template"
//@ diag: 14:32 error "'E3' cannot be defined in a type alias template"
//@ diag: 15:50 error "'N::S4' cannot be defined in a type alias template"
template<class T> using A = struct { int v; };
template<class T> using B = struct S2 { int v; };
template<int N> using C = enum E3 { e3 = N };
namespace N { template<class T> using D = struct S4 { T t; } *; }
