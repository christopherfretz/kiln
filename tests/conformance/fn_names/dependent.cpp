// Function-name spans (spec/ast.md §24.4): a dependent name in a type the
// pattern writes is an entry with no entity, `dep`; its non-dependent
// parts keep theirs. A dependent `decltype` return prints
// `<dependent type>` and gets none.
//@ fn-names: dependent.names
//@ std: c++20
namespace ns {
struct S { using type = int; static int v; };
template <class T> struct W { using type = T; };
template <class T> void dep(typename W<T>::type) { (void)__PRETTY_FUNCTION__; }
template <class T> auto dt(T t) -> decltype(t.v) { (void)__PRETTY_FUNCTION__; return 0; }
template <class T> void mem(typename T::type) { (void)__PRETTY_FUNCTION__; }
}
void use() { ns::dep<ns::S>({}); ns::dt(ns::S{}); ns::mem<ns::S>(1); }
