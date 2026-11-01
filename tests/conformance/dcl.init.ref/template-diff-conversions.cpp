// [dcl.init.ref]/5, [dcl.init.general]/16, [expr.ass]/3, [expr.cond]/7, [dcl.typedef]/3,
// [basic.link]/11, [dcl.type.auto.deduct]/4, [class.virtual]/8: diagnostics that name a source and a
// target type print clang's elided template diff when both are specializations of one template
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:16 error "binding reference of type 'pr<...>' to value of type 'const pr<...>' drops 'const' qualifier"
//@ diag: 30:16 error "non-const lvalue reference to type 'pr<[...], char>' cannot bind to a temporary of type 'pr<[...], int>'"
//@ diag: 31:17 error "rvalue reference to type 'pr<...>' cannot bind to lvalue of type 'pr<...>'"
//@ diag: 32:15 error "no viable conversion from 'pr<[...], int>' to 'pr<[...], char>'"
//@ diag: 26:35 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'pr<int, int>' to 'const pr<int, char> &' for 1st argument"
//@ diag: 26:35 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'pr<int, int>' to 'pr<int, char> &&' for 1st argument"
//@ diag: 33:11 error "conversion function from 'int' to 'vec<char>' invokes a deleted function"
//@ diag: 27:32 note "'vec' has been explicitly marked deleted here"
//@ diag: 35:22 error "typedef redefinition with different types ('pr<[...], int>' vs 'pr<[...], char>')"
//@ diag: 34:23 note "previous definition is here"
//@ diag: 37:21 error "redeclaration of 'e1' with a different type: 'pr<[...], int>' vs 'pr<[...], char>'"
//@ diag: 36:22 note "previous declaration is here"
//@ diag: 38:1 error "'auto' deduced as 'pr<int, char>' in declaration of 'a1' and deduced as 'pr<int, int>' in declaration of 'a2'"
//@ diag: 40:11 error "deduced conflicting types ('pr<[...], char>' vs 'pr<[...], int>') for initializer list element type"
//@ diag: 42:29 error "virtual function 'f' has a different return type ('pr<[...], int>') than the function it overrides (which has return type 'pr<[...], char>')"
//@ diag: 41:34 note "overridden virtual function is here"
//@ diag: 45:5 error "no viable overloaded '='"
//@ diag: 26:35 note "candidate function (the implicit copy assignment operator) not viable: no known conversion from 'pr<[...], int>' to 'const pr<[...], char>' for 1st argument"
//@ diag: 26:35 note "candidate function (the implicit move assignment operator) not viable: no known conversion from 'pr<[...], int>' to 'pr<[...], char>' for 1st argument"
//@ diag: 46:5 error "incompatible operand types ('pr<[...], char>' and 'pr<[...], int>')"
template<class A, class B> struct pr {};
template<class T> struct vec { vec(int) = delete; };
const pr<int, char> c{};
pr<int, char> &r1 = c;
pr<int, char> &r2 = pr<int, int>{};
pr<int, char> &&r3 = r1;
pr<int, char> x = pr<int, int>{};
vec<char> v = 1;
typedef pr<int, char> T1;
typedef pr<int, int> T1;
extern pr<int, char> e1;
extern pr<int, int> e1;
auto a1 = pr<int, char>{}, a2 = pr<int, int>{};
#include <initializer_list>
auto il = {pr<int, char>{}, pr<int, int>{}};
struct B { virtual pr<int, char> f(); };
struct E : B { pr<int, int> f() override; };
void g(bool b) {
  pr<int, char> l;
  l = pr<int, int>{};
  b ? pr<int, char>{} : pr<int, int>{};
}
