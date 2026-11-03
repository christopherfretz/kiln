// [dcl.fct]/6 (CWG1417): a qualified function type named through a
// typedef-name in a type-id or parameter: a trailing return type, a
// `sizeof` or `new` operand, a handler's, a template's or a lambda's
// parameter, a parenthesized declarator, is an error at the type-id (the
// qualifiers are dropped, so the follow-on errors print `int ()`); as a
// template argument, a type trait's operand, an alias-declaration, a
// typedef, a type parameter's default or a pointer to member's pointee it
// is allowed. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:14 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 23:1 error "function cannot return function type 'int ()'"
//@ diag: 24:16 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 24:9 error "invalid application of 'sizeof' to a function type"
//@ diag: 25:15 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 25:15 error "cannot allocate function type 'int ()' with new"
//@ diag: 26:26 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 27:11 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 28:1 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 29:15 error "non-member function of type 'F' (aka 'int () const') cannot have 'const' qualifier"
typedef int F() const;
struct S { F ok; };
auto r2() -> F;
int x = sizeof(F);
auto vv = new F;
void n() { try {} catch (F f) {} }
template <F x> struct TP {};
F (g2);
auto lam = [](F p) {};
using A = F;
typedef F B;
template <class T> struct Y {};
Y<F> y;
template <class T = F> struct D {};
F S::*pm;
using P = F S::*;
bool same = __is_same(F, A);
int rank = __array_rank(F);
