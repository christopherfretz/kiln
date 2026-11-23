// Per-specialization entries (spec/ast.md §18.9): members of a dependent
// base class are not found by unqualified lookup in the template
// definition ([temp.dep.general]/4), so they are written `this->m` or
// `Base<T>::m` and looked up at instantiation in the specialization of
// the base. The member named is the base specialization's member (its
// pattern is reached through `template_info`).
//@ written-refs: inst-this-base.refs
template <class T> struct Base {
  T stored;
  T fetch() const { return stored; }
  static T zero() { return T(); }
};

template <class T> struct Derived : Base<T> {
  T twice() const { return this->stored + this->fetch() + Base<T>::zero(); }
};

int run() {
  Derived<int> d{};
  return d.twice();
}
