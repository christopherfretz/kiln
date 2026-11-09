// [basic.lookup.argdep]/3: an argument '&g' naming an overload set (here
// unparenthesized, with a function template member) has the associated
// entities of every member; g(N1::X) makes N1 associated, so h is found.
// WP-958 sibling of address-of-overload-set.cpp.
//@ std: c++20
//@ expect: compile-ok
namespace N1 {
struct X {};
int h(int (*)(void *));
} // namespace N1
int g(void *);
template <class T> long g(T, N1::X);
int test() { return h(&g); }
