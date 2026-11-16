// [class.friend]/4, [temp.friend]/1: `friend Z<T>::Z(long);` in a class
// template is matched in each specialization: when the class the qualifier
// names has no constructor with that parameter-type-list the friend is
// "friend declaration of 'Z' does not match any declaration in 'Z<int>'",
// with clang's candidate notes for the constructors found (clang's own
// wording for a non-dependent class; found running GCC's C++ testsuite, as
// GCC reports it there too). Only the matching member is befriended:
// `Q<int>::Q(int)` and `Q<int>::~Q()` are friends, `Q<int>::Q(long)` is not
// (the -run sibling shows the friends' access).
//@ skip(clang): clang befriends no constructor or destructor of a dependent class and reports no mismatch
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:16 error "friend declaration of 'Z' does not match any declaration in 'Z<int>'"
//@ diag: 26:8 note "in instantiation of template class 'Y<int>' requested here"
//@ diag: 21:31 note "candidate constructor has type mismatch at 1st parameter (expected 'long' but has 'int')"
//@ diag: 21:27 note "candidate constructor (the implicit copy constructor) has type mismatch at 1st parameter (expected 'long' but has 'const Z<int> &')"
//@ diag: 36:48 error "'x' is a private member of 'V<int>'"
//@ diag: 37:14 note "in instantiation of member function 'Q<int>::Q' requested here"
//@ diag: 31:14 note "implicitly declared private here"

template <class T> struct Z { Z(int); ~Z(); };
template <class T> class Y {
  friend Z<T>::Z(long);
  static int x;
};
Y<int> y;
template <class T> struct Q { Q(int); Q(long); ~Q(); };
template <class T> class V {
  friend Q<T>::Q(T);
  friend Q<T>::~Q();
  static int x;
};
template <class T> int V<T>::x = 0;
template <class T> Q<T>::Q(int) {}
template <class T> Q<T>::~Q() {}
template <class T> Q<T>::Q(long) { (void)V<T>::x; }
Q<int> a(1), b(2L);
