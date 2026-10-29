// [namespace.udecl]/11 and [basic.scope.declarative]: redeclaring the same
// entity (at namespace or block scope), a typedef of the same type, or a
// class name beside a non-class name is not a conflict
//@ expect: compile-ok
//@ strict-diags
typedef int T;
namespace N { typedef int T; using ::T; int x; void f(int); struct S {}; }
using N::T;
using N::x;
using N::x;
using N::f;
using N::f;
int g() { using N::f; using N::f; f(1); return x; }
using N::S;
struct S *p;
int S2;
namespace R { struct S2 {}; }
using R::S2;
T t = x;
