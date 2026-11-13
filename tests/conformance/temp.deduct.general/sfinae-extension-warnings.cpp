// [temp.deduct.general]/8: in a SFINAE context (here a requires-expression's
// substitution), three of clang's extension warnings are substitution
// failures, as clang 19 marks them `SFINAEFailure`; outside one they are
// warnings: a rewritten `!=` whose `operator==` does not return `bool`
// ([over.match.oper]/9), too many braces around a scalar initializer
// ([dcl.init.list]/3.10), and brace elision in a designated initializer
// ([dcl.init.aggr]/4.2).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:12 warning "ISO C++20 requires return type of selected 'operator==' function for rewritten '!=' comparison to be 'bool', not 'int'"
//@ diag: 15:16 note "declared here"
//@ diag: 25:9 warning "too many braces around scalar initializer"
//@ diag: 26:12 warning "brace elision for designated initializer is a C99 extension"
struct A { int operator==(const A &) const; };
struct P { struct Q { int x, y; } q; int z; };
template <class T> concept NotEq = requires(T a) { a != a; };
template <class T> concept Braced = requires { T{{1}}; };
template <class T> concept Elided = requires { T{.q = 1, .z = 3}; };
static_assert(!NotEq<A>);
static_assert(!Braced<int>);
static_assert(!Elided<P>);
void f(A a) {
  (void)(a != a);
  int i{{1}};
  P p{.q = 1, .z = 3};
  (void)i;
  (void)p;
}
