// [temp.expl.spec]/16: member templates of implicit class template
// specializations specialized as templates
// (`template<> template<class U> int A<long>::g(U)`), WP-117.
template<class T> struct A {
  template<class U> int g(U) { return 1; }
  template<class U> static int h(U);
  template<class U> int k(U, T) { return 3; }
};
template<> template<class U> int A<long>::g(U u) { return 2 + sizeof(U); }
template<> template<class U> int A<char>::h(U) { return 7; }
template<class T> template<class U> int A<T>::h(U) { return 5; }
template<> template<class U> int A<short>::k(U, short) { return 4; }
int f() { A<long> a; A<int> b; A<short> s; return a.g(1) * 10 + b.g(1) + A<char>::h(1) + A<int>::h(1) + s.k(1, 1) + b.k(1, 1); }
