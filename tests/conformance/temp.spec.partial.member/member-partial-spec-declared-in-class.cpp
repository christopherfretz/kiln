// [temp.spec.partial.member]/1, [temp.spec.partial.general]/1: a partial specialization of a
// member class template declared in the class and defined out of line; the definition completes
// the in-class declaration and the declarations after it are back at namespace scope
//@ expect: compile-ok
//@ strict-diags
template <class T>
struct O2 {
  template <class U>
  struct In;
  template <class U>
  struct In<U&>;
};
template <class T>
template <class U>
struct O2<T>::In<U&> {
  static constexpr int v = sizeof(T) + 10;
};
static_assert(O2<long>::In<int&>::v == 18);
static_assert(sizeof(O2<char>) == 1);
int main() {}
