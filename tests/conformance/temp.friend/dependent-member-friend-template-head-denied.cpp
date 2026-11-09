// [temp.friend]/5, [temp.over.link]/4-6: `template <class T> template
// <class V> friend int A<T>::h(const B&);` befriends a member function
// template of each A<T> only when the friend's own template head is
// equivalent to the member template's and the types match after the
// friend's parameters are renamed to the member's (WP-1098). A head with
// another parameter count (h2) or kind (h3), a different signature (h4), a
// friend without a head of its own naming a member template (h5), and a
// friend with a head naming a non-template member (h6) are denied; h is
// befriended. clang turns off access control for B instead
// (-Wunsupported-friend) and accepts the TU (an accepts-invalid; g++ 14
// rejects each mismatching friend).
//@ skip(clang): clang turns off access control for the befriending class
//@ expect: compile-fail
//@ strict-diags
class B;
template <class T> struct A {
  template <class V> static int h(const B &b);
  template <class V, class W> static int h2(const B &b);
  template <int N> static int h3(const B &b);
  template <class V> static int h4(V, const B &b);
  template <class V> static int h5(int, const B &b);
  static int h6(const B &b);
};
class B {
  template <class T> template <class V> friend int A<T>::h(const B &);
  template <class T> template <class V> friend int A<T>::h2(const B &);
  template <class T> template <class V> friend int A<T>::h3(const B &);
  template <class T> template <class V> friend int A<T>::h4(int, const B &);
  template <class T> friend int A<T>::h5(int, const B &);
  template <class T> template <class V> friend int A<T>::h6(const B &);
  int n = 7;
};
template <class T> template <class V> int A<T>::h(const B &b) { return b.n; }
template <class T> template <class V, class W> int A<T>::h2(const B &b) { return b.n; }
template <class T> template <int N> int A<T>::h3(const B &b) { return b.n; }
template <class T> template <class V> int A<T>::h4(V, const B &b) { return b.n; }
template <class T> template <class V> int A<T>::h5(int, const B &b) { return b.n; }
template <class T> int A<T>::h6(const B &b) { return b.n; }
int main() {
  B b;
  return A<int>::h<char>(b) + A<int>::h2<char, int>(b) + A<int>::h3<1>(b) + A<int>::h4(1, b) +
         A<int>::h5<char>(1, b) + A<int>::h6(b);
}
