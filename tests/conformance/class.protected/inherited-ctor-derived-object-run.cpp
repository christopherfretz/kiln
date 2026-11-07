// [namespace.udecl]/19, [class.protected]/1, [class.inhctor.init]: a
// protected base constructor inherited by a using-declaration constructs
// an object of the derived class, so a member of the derived class may
// call it (direct-, copy-list- and functional-cast initialization), also
// in a class template whose base is dependent (a bundled test-framework
// header's `BasicResult::ok`, WP-683). Matches clang 19.
//@ expect: run
struct B {
protected:
  B(int t) : m(t) {}
  B(int t, int u) : m(t + u) {}
  int m;

public:
  int get() const { return m; }
};
struct D : B {
  static D list() { return {1}; }
  static D cast() { return D(2, 3); }
  static D direct() {
    D d(4);
    return d;
  }

protected:
  using B::B;
};
template <class T> struct V : B {
protected:
  using B::B;
};
template <class T> struct R : V<T> {
  template <class U> static R ok(U v) { return {v, 1}; }
  static R ok() { return {7}; }

protected:
  using V<T>::V;
};
int main() {
  int s = D::list().get() + D::cast().get() + D::direct().get();
  s += R<int>::ok(2).get() + R<long>::ok().get();
  return s == 1 + 5 + 4 + 3 + 7 ? 0 : 1;
}
