// [basic.lookup.argdep]/3: for an argument naming a set of overloaded
// functions, the associated entities are those of every member's parameter
// and return types; g(N1::X) makes N1 associated, so h is found. From
// clang's CXX/basic/basic.lookup/basic.lookup.argdep/p2-template-id.cpp.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
namespace N1 {
struct X {};
void h(int (*)(void *));
} // namespace N1
int g(void *);
long g(N1::X);
void test() { h((&g)); }
