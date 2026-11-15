// GNU used on a destructor of a class template. The attribute is not an
// odr-use, and the destructor's exception specification is a complete-class
// context ([class.mem.general]/7, [except.spec]/13), so instantiating the
// class is valid; GCC 16.1 accepts. clang 19 and 22 reject it (they mark the
// destructor referenced while the class is still incomplete).
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 and 22 reject: exception specification needed for member of incomplete class
template <class T> struct A {
  __attribute__((used)) ~A() {}
};
A<int> a;
template <class T> struct B { __attribute__((used)) ~B(); };
template <class T> B<T>::~B() {}
B<int> b;
template struct A<long>;
int main() { return 0; }
