// Module `mfr`: a class template W<T> befriending constrained and
// abbreviated member function templates of A<T> through a qualifier that
// names W's parameter (matched when W is instantiated, WP-1110/WP-1145),
// and a class B befriending a member function template of every A<T>
// ([temp.friend]/5, WP-1130).
export module mfr;
export template <class T> concept C = sizeof(T) > 1;
export template <class T> class W;
export class B;
export template <class T> struct A {
  template <C U> static int f(U, W<T> &);
  static int j(C auto, W<T> &);
  template <class U> static int o(U, W<T> &) requires C<U>;
  template <class U> static int o(U, W<T> &);
  template <C U> static int b(U, const B &);
};
export template <class T> class W {
  int s = 3;
  template <C U> friend int A<T>::f(U, W<T> &);
  friend int A<T>::j(C auto, W<T> &);
  template <class U> friend int A<T>::o(U, W<T> &) requires C<U>;
};
export class B {
  template <class T> template <C U> friend int A<T>::b(U, const B &);
  int n = 5;
public:
  B() {}
};
template <class T> template <C U> int A<T>::f(U, W<T> &w) { return w.s; }
template <class T> int A<T>::j(C auto, W<T> &w) { return w.s + 1; }
template <class T> template <class U> int A<T>::o(U, W<T> &w) requires C<U> { return w.s + 2; }
template <class T> template <class U> int A<T>::o(U, W<T> &w) {
  if constexpr (requires { w.s; }) return w.s;
  else return 0;
}
template <class T> template <C U> int A<T>::b(U, const B &x) { return x.n; }
