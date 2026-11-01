// [temp.mem]/1, [temp.mem.class]/1: a member class template defined outside its enclosing
// class template takes one template head per enclosing template, outermost first
//@ expect: run
//@ strict-diags
template <class T>
struct Outer {
  template <class U>
  struct Inner;
};
template <class T>
template <class U>
struct Outer<T>::Inner {
  T t;
  U u;
  int sum() const { return int(t) + int(u); }
  static int st;
};
template <class T>
template <class U>
int Outer<T>::Inner<U>::st = sizeof(T) + sizeof(U);
int main() {
  Outer<long>::Inner<char> i{40, 2};
  return (i.sum() == 42 ? 0 : 1) + (Outer<long>::Inner<char>::st == 9 ? 0 : 2);
}
