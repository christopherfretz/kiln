// The written-reference table (spec/ast.md §18.5): a variable template-id
// whose template name is not dependent and whose arguments are
// (`n::v<T>`) names the variable template, which lookup at the
// definition already resolved: a variable template is never overloaded,
// so only the specialization waits for the arguments ([temp.names]/2,
// [temp.dep.expr]/3). The same holds for a member variable template of
// a class that is not dependent (`n::S::w<T>`, `n::D<int>::w<T>`) and
// for one found through a using-declaration; one of a dependent class
// (`n::D<T>::template w<T>`) stays dependent. A function template-id is
// an overload set and stays dependent with its found set (§18.12), as in
// `found-template-id`.
//@ written-refs: var-template-id.refs
namespace n {
template <class T> constexpr bool v = true;
struct S {
  template <class U> static constexpr int w = 1;
};
template <class T> struct D {
  template <class U> static constexpr int w = 2;
};
template <class T> int g(T) { return 0; }
} // namespace n

template <class T> bool k() { return n::v<T>; }
template <class T> int m() {
  return n::S::w<T> + n::D<T>::template w<T> + n::D<int>::w<T>;
}
template <class T> bool never() {
  using n::v;
  return v<T *> && n::g<T>(T());
}
bool z = k<int>() && m<long>() > 0;
