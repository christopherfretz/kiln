// [temp.over.link]/5, [temp.dep.candidate]: an unresolved (ADL) call in a
// return type compares by name and arguments, not by the declarations
// found at the point of definition.
namespace n {
struct S {};
int g(S);
} // namespace n
template <class T> auto f(T t) -> decltype(g(t));
template <class U> auto f(U u) -> decltype(g(u)) { return g(u); }
int r = f(n::S());
