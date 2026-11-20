// [temp.friend]/5, [class.access]: member friends named by an
// operator-function-id with a dependent qualifier grant every
// specialization's operator access (`b(c)`, `b + c`, `operator new` and
// `operator delete` of B<long>). clang 22 does not model them
// (-Wunsupported-friend, then denies the access); GCC 16.1 grants it.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 does not model [temp.friend]/5 friends (-Wunsupported-friend, access denied)
//@ stdout: 41 42 1 0
#include <cstdio>
template<class> class C;
template<class T> struct B {
  int operator()(const C<int>& c) const;
  int operator+(const C<int>& c) const;
  static void* operator new(__SIZE_TYPE__ n);
  static void operator delete(void* p);
  int v = 1;
};
template<class U> class C {
  int priv = 40;
  template<class T> friend int B<T>::operator()(const C<int>&) const;
  template<class T> friend int B<T>::operator+(const C<int>&) const;
  template<class T> friend void* B<T>::operator new(__SIZE_TYPE__);
  template<class T> friend void B<T>::operator delete(void*);
  static int count;
public:
  static int live() { return count; }
};
template<class U> int C<U>::count = 0;
template<class T> int B<T>::operator()(const C<int>& c) const { return c.priv + v; }
template<class T> int B<T>::operator+(const C<int>& c) const { return c.priv + 2; }
template<class T> void* B<T>::operator new(__SIZE_TYPE__ n) { ++C<int>::count; return ::operator new(n); }
template<class T> void B<T>::operator delete(void* p) { --C<int>::count; ::operator delete(p); }
int main() {
  C<int> c;
  B<char> b;
  B<long>* p = new B<long>;
  int n = C<int>::live();
  std::printf("%d %d %d ", b(c), b + c, n);
  delete p;
  std::printf("%d\n", C<int>::live());
}
