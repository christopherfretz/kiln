// [temp.inst]/7, [temp.static]: the instantiation of a variable template
// specialization that is a class member is noted as a static data member
// (clang `PrintInstantiationStack`), at namespace scope as a variable
// template specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:55 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 20:12 note "in instantiation of static data member 'S::v<int>' requested here"
//@ diag: 21:37 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:9 note "in instantiation of variable template specialization 'w<int>' requested here"
//@ diag: 24:42 error "cannot initialize a variable of type 'const int' with an rvalue of type 'int *'"
//@ diag: 28:18 note "in instantiation of static data member 'X<char>::p<int>' requested here"
//@ diag: 27:57 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:18 note "in instantiation of static data member 'X<char>::q<int>' requested here"
//@ diag: 31:46 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 34:19 note "in instantiation of static data member 'Y<short>::r<short>' requested here"
//@ diag: 32:58 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 35:19 note "in instantiation of static data member 'Y<short>::s<int>' requested here"
struct S { template<class U> static constexpr int v = U::bad; };
int b = S::v<int>;
template<class U> constexpr int w = U::bad;
int c = w<int>;
template<class T> struct X {
  template<class U> static constexpr int p = (U*)0;
  template<class U> static const int q;
};
template<class T> template<class U> const int X<T>::q = U::bad;
int d = X<char>::p<int>;
int e = X<char>::q<int>;
template<class T> struct Y {
  template<class U> static constexpr int r = T::bad + U::bad;
  template<class U> static constexpr int s = sizeof(T) + U::bad;
};
int f = Y<short>::r<short>;
int g = Y<short>::s<int>;
