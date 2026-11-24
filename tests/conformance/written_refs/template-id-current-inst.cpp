// The written-reference table (spec/ast.md §18.5, §18.12.2): a member
// variable template of the current instantiation named with dependent
// template arguments ([temp.dep.type]/1, /6). Class member lookup at the
// definition found it, and a variable template is never overloaded, so
// in a class without a dependent base the name names the template:
// unqualified, after the class's own name and after `this->`, with or
// without `template`, also when a using-declaration brought it in from a
// base that is not dependent (`found=` the using-shadow). With a
// dependent base the instantiation looks a `this->` name up again
// (§18.12.2's rule for members of the current instantiation): it stays
// dependent with its found set and `+base`; a name the class's own
// lookup finds unqualified or after the class's name is resolved there.
// A member function template stays an overload set with its found set.
// A member template of an unknown specialization (`t.template w<T>`,
// `D<T>::template w<T>`) has no definition-context lookup. A partial
// specialization's member is its own, not the primary's.
//@ written-refs: template-id-current-inst.refs
struct P {
  template <class U> static constexpr int pw = 3;
};
template <class T> struct D {
  template <class U> static constexpr int w = 2;
};
template <class T> struct S : P {
  template <class U> static constexpr int mw = 0;
  template <class U> static int mf() { return 0; }
  using P::pw;
  int a() { return mw<T> + S::mw<T> + this->mw<T> + this->template mw<T>; }
  int b(D<T> t) {
    return this->pw<T> + this->template mf<T>() + t.template w<T> + D<T>::template w<T>;
  }
};
template <class T> struct S<T *> {
  template <class U> static constexpr int mw = 1;
  int a() { return this->template mw<T>; }
};
template <class T> struct R : D<T> {
  template <class U> static constexpr int mw = 0;
  int a() { return this->template mw<T> + R::template mw<T> + mw<T>; }
};
int z = S<int>().a() + S<int>().b(D<int>()) + S<int *>().a() + R<int>().a();
