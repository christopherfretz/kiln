// [namespace.udecl]/19, [class.protected]/1: a member using-declarator in a
// class template names a protected member of an indirect base; the path to it
// goes through a dependent base, so the check made on the template is redone
// in each instantiation, through an object of the instantiated class (not of
// the template) — which derives from the base, so the member is accessible.
// Nested classes of a class template replay with their own instantiation.
//@ expect: run
//@ strict-diags
class B {
protected:
  int m;
  int f() const { return m + 1; }

public:
  B(int v) : m(v) {}
};
template <class T> class V : public B {
public:
  V(int v) : B(v) {}
};
template <class T> class R : public V<T> {
public:
  R(int v) : V<T>(v) {}
  using B::m;
  using B::f;
};
template <class T> struct O {
  struct N : V<T> {
    N(int v) : V<T>(v) {}
    using B::m;
  };
};
int main() {
  R<int> r(1);
  O<char>::N n(2);
  return r.m + r.f() + n.m - 5;
}
