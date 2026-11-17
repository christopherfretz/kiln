// [temp.friend]/1: a friend class template named with an elaborated
// type specifier, and a friend specialization of a function template.
//@ expect: compile-ok
//@ strict-diags
template<int> void foo();
template<int T> struct S { };
template<class T> struct W {
  template<class U> friend struct W;
  template<int U> friend struct S;
  friend void foo<0>();
};
W<int> w;
