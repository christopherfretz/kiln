// [temp.expl.spec]/16: a member template of an implicitly instantiated
// class template specialization specialized as a template
// (`template<> template<class U> struct A<int>::B`): its specializations
// are instantiated from the new definition, with its own arguments only.
//@ expect: compile-ok
//@ strict-diags
template<class T> struct Outer {
  template<class U> struct Inner { static constexpr int v = 1; };
  template<class U> static constexpr int g(U) { return 1; }
  template<class U> constexpr int h(U) const;
  template<class U> static constexpr int vt = 1;
};
template<class T> template<class U> constexpr int Outer<T>::h(U) const { return 10; }
template<> template<class U> struct Outer<int>::Inner { static constexpr int v = 2; U u; };
template<> template<class U> struct Outer<long>::Inner<U*> { static constexpr int v = 3; };
template<> template<class U> constexpr int Outer<int>::g(U) { return sizeof(U); }
template<> template<class U> constexpr int Outer<int>::h(U) const { return 20; }
template<> template<class U> constexpr int Outer<int>::vt = 2;
static_assert(Outer<int>::Inner<char>::v == 2 && Outer<char>::Inner<char>::v == 1);
static_assert(Outer<long>::Inner<int*>::v == 3 && Outer<long>::Inner<int>::v == 1);
Outer<int>::Inner<long> x{3};
static_assert(sizeof(x) == sizeof(long));
static_assert(Outer<int>::g(1.0) == 8 && Outer<char>::g(1) == 1);
static_assert(Outer<int>{}.h(1) == 20 && Outer<char>{}.h(1) == 10);
static_assert(Outer<int>::vt<char> == 2 && Outer<char>::vt<char> == 1);
