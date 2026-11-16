// [temp.param]/12, [class.access.general]/7: the names in a default
// template argument are checked in the context of the template that
// declares it, not where the default is used: a friend template of the
// class and a member template see its private names, a use of the
// default from outside does not need to. Found running GCC's C++
// testsuite.
//@ expect: run
//@ strict-diags
class A {
  typedef int X;
  template<class T> friend struct B;
  template<class T> friend int g();
public:
  template<class T = X> struct In { T t; };
};
template<class T = A> struct B { typename T::X x; };
template<class T = A> int g() { return sizeof(typename T::X); }
template<class T, class U = typename T::X> struct BB;
B<> b;
A::In<> in;
int main() { return g() == sizeof(int) && sizeof(in.t) == sizeof(int) ? 0 : 1; }
