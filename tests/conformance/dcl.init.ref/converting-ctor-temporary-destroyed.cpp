// [dcl.init.ref]/5.4.1, [class.temporary]/4: a reference bound to a
// temporary created by a converting constructor destroys it, at the end
// of the full-expression for a parameter and with the reference for a
// local. The class's destructor is implicit and non-trivial (libc++
// `optional<T>` from a `U&&` argument, optional.object.ctor/U.pass.cpp).
//@ expect: run
int dtors = 0;
struct T {
  int v;
  T(int x) : v(x) {}
  ~T() { ++dtors; }
};
template <class X> struct B {
  X x;
  template <class U> B(U&& u) : x(u) {}
};
template <class X> struct O : B<X> {
  template <class U> O(U&& u) : B<X>(u) {}
};
bool g(O<T>&& o) { return o.x.v == 3 && dtors == 0; }
int main() {
  bool ok = g(3); // the temporary dies at the `;`
  if (!ok || dtors != 1)
    return 1;
  {
    const O<T>& r = 4;
    if (r.x.v != 4 || dtors != 1)
      return 2;
  }
  return dtors == 2 ? 0 : 3;
}
