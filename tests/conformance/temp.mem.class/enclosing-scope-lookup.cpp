// [class.mem]/4, [basic.scope.class]: names in the body of a member class defined out of line
// (`struct A::B { ... }`) are looked up in the enclosing class too, for templates and
// non-templates alike
//@ expect: run
//@ strict-diags
struct P {
  typedef int value_type;
  static constexpr int k = 4;
  struct B;
};
struct P::B {
  value_type v = k;
};
template <class T>
struct Q {
  typedef T value_type;
  static constexpr int k = 5;
  struct B;
};
template <class T>
struct Q<T>::B {
  value_type v = k;
};
int main() {
  P::B p;
  Q<int>::B q;
  return (p.v == 4 ? 0 : 1) + (q.v == 5 ? 0 : 2);
}
