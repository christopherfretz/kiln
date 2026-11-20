// [class.friend]/2, /3: a friend of a class template is a class or a
// function. A variable, a variable template specialization or a typedef
// is rejected once, in the template definition; instantiating the class
// declares no friend for them and reports nothing again. The -run sibling
// instantiates every kind of friend.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 16:14 error "friends can only be classes or functions"
//@ diag: 17:10 error "'typedef' is invalid in friend declarations"
//@ diag: 17:22 error "friends can only be classes or functions"
//@ diag: 18:14 error "friends can only be classes or functions"

template <class U> int vt = 0;
template <class T> struct A {
  friend int vt<int>;
  friend typedef int I;
  friend int x;
  T v;
};
A<int> a;
A<char> b;
