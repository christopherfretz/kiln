//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [temp.mem]/1: member templates are redeclared in each specialization of
// the enclosing class template.
template <class T> struct A {
  template <class U> struct B { T t; U u; };
  template <class U> using X = T *;
};
A<int>::B<char> *b;
int s = sizeof(A<int>::B<char>);
A<int>::X<long> x;
