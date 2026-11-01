// [temp.spec.partial.member]/1, [temp.mem.class]/1: a member class of a class template
// partial specialization defined out of line uses the partial specialization's template head
//@ expect: run
//@ strict-diags
template <class T>
struct A {
  static constexpr int which = 0;
};
template <class T>
struct A<T*> {
  struct B;
  template <class U>
  struct BT;
};
template <class T>
struct A<T*>::B {
  T x = 7;
};
template <class T>
template <class U>
struct A<T*>::BT {
  int f() { return sizeof(T) * 10 + sizeof(U); }
};
int main() {
  A<int*>::B b;
  A<long*>::BT<char> bt;
  return (b.x == 7 ? 0 : 1) + (bt.f() == 81 ? 0 : 2);
}
