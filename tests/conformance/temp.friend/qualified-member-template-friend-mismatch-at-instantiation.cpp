// [temp.friend]/1, [temp.over.link]/6-7, [dcl.meaning.general]/2.3: a
// friend member function template whose qualifier names the enclosing
// class's parameter (`template <C U> friend int A<T>::f(U, W<T> &);` in
// W<T>) is matched when W is instantiated (WP-1110, WP-1145), and it names
// no member of A<int> when the constraints are not equivalent: a
// type-constraint only the friend has (f), another concept (g), a trailing
// requires-clause only the member has (h), `C auto` against `auto` (i) and
// the converse (j), or another parameter list (k). GCC 16.1 diagnoses these
// too; clang drops such a friend template at instantiation without a
// diagnostic (it does diagnose a non-template one, see
// qualified-friend-mismatch-at-instantiation.cpp).
//@ skip(clang): clang drops a non-matching dependent-qualified friend template at instantiation silently
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:35 error "friend declaration of 'f' does not match any declaration in 'A<int>'"
//@ diag: 41:8 note "in instantiation of template class 'W<int>' requested here"
//@ diag: 35:35 error "friend declaration of 'g' does not match any declaration in 'A<int>'"
//@ diag: 36:39 error "friend declaration of 'h' does not match any declaration in 'A<int>'"
//@ diag: 37:20 error "friend declaration of 'i' does not match any declaration in 'A<int>'"
//@ diag: 38:20 error "friend declaration of 'j' does not match any declaration in 'A<int>'"
//@ diag: 39:39 error "friend declaration of 'k' does not match any declaration in 'A<int>'"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept E = sizeof(T) > 2;
template <class T> class W;
template <class T> struct A {
  template <class U> static int f(U, W<T> &);
  template <C U> static int g(U, W<T> &);
  template <class U> static int h(U, W<T> &) requires C<U>;
  static int i(auto, W<T> &);
  static int j(C auto, W<T> &);
  template <class U> static int k(U, W<T> &);
};
template <class T> class W {
  template <C U> friend int A<T>::f(U, W<T> &);
  template <E U> friend int A<T>::g(U, W<T> &);
  template <class U> friend int A<T>::h(U, W<T> &);
  friend int A<T>::i(C auto, W<T> &);
  friend int A<T>::j(auto, W<T> &);
  template <class U> friend int A<T>::k(U, W<T> &, int);
};
W<int> w;
