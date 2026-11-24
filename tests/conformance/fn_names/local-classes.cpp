// Function-name spans (spec/ast.md §24.4): a function in the scope (the
// member of a local class) is a scope entry naming the function, the
// names of its printed parameters one depth in.
//@ fn-names: local-classes.names
//@ std: c++20
namespace ns {
struct S {};
void loc(S) {
  struct L { void m() { (void)__PRETTY_FUNCTION__; } };
  L{}.m();
}
template <class T> void tl(T) {
  struct M { static void m() { (void)__PRETTY_FUNCTION__; } };
  M::m();
}
}
void use() { ns::loc({}); ns::tl(ns::S{}); }
