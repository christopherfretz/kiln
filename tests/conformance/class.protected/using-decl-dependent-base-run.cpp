// [namespace.udecl]/19, [class.protected]/1, [temp.dep.type]: a class
// template whose base is dependent may redeclare a protected member of an
// indirect base with a using-declaration; the access check waits for the
// instantiation, where the member is named through an object of the
// instantiated class (not of the pattern), so it is accessible. Also for a
// member class of the template and a protected member function (a bundled
// test-framework header's `using ResultBase::m_type;`, WP-683). Matches
// clang 19.
//@ expect: run
class B {
protected:
  int m = 2;
  int g() const { return 3; }
};
template <class T> struct V : B {};
template <class T> struct R : V<T> {
  using B::m;
  using B::g;
  struct In : V<T> {
    using B::m;
  };
};
int main() {
  R<int> r;
  R<int>::In in;
  return r.m + r.g() + in.m == 7 ? 0 : 1;
}
