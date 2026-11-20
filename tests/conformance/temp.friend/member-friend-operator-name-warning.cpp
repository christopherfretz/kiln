// [temp.friend]/5, [over.oper.general]/1: a member friend named by an
// operator-function-id with a dependent qualifier (`template <class T>
// friend void *A<T>::operator new(std::size_t);`) is matched by its name
// as any other member friend; a deallocation function's implicit
// `noexcept` does not keep it from matching ([except.spec]/9). clang 22
// warns -Wunsupported-friend for each, at the operator name; kiln keeps
// the text (the clang 19 spelling differs).
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 19:44 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'A'"
//@ diag: 20:43 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'A'"
//@ diag: 30:41 warning "dependent nested name specifier 'B<T>' for friend class declaration is not supported; turning off access control for 'C'"
//@ diag: 31:39 warning "dependent nested name specifier 'B<T>' for friend class declaration is not supported; turning off access control for 'C'"
//@ diag: 32:40 warning "dependent nested name specifier 'B<T>' for friend class declaration is not supported; turning off access control for 'C'"
template <typename> struct A {
  static void *operator new(__SIZE_TYPE__);
  static void operator delete(void *);
  template <typename T> friend void *A<T>::operator new(__SIZE_TYPE__);
  template <typename T> friend void A<T>::operator delete(void *);
};
A<int> a;
template <class T> struct B {
  void *operator new[](__SIZE_TYPE__);
  int operator+(int) const;
  bool operator==(const B &) const;
};
template <class U> class C {
  int priv = 3;
  template <class T> friend void *B<T>::operator new[](__SIZE_TYPE__);
  template <class T> friend int B<T>::operator+(int) const;
  template <class T> friend bool B<T>::operator==(const B<T> &) const;
};
C<int> c;
