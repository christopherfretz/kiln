// [temp.expl.spec]/7: an explicit specialization of a member template of
// an implicit class template specialization declared after its first use.
// clang names a class by its fully scoped type, a function unqualified
// with its arguments and a variable template by its qualified name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:44 error "explicit specialization of 'n::Z<char>::M<int>' after instantiation"
//@ diag: 29:20 note "implicit instantiation first required here"
//@ diag: 32:41 error "explicit specialization of 'f<int>' after instantiation"
//@ diag: 31:21 note "implicit instantiation first required here"
//@ diag: 34:41 error "explicit specialization of 'n::Z<char>::v' after instantiation"
//@ diag: 33:21 note "implicit instantiation first required here"
//@ diag: 36:23 error "explicit specialization of 'n::Y<int>' after instantiation"
//@ diag: 35:11 note "implicit instantiation first required here"
//@ diag: 38:20 error "explicit specialization of 'g<int>' after instantiation"
//@ diag: 37:12 note "implicit instantiation first required here"
//@ diag: 42:40 error "explicit specialization of 'P<int>::M<int *>' after instantiation"
//@ diag: 41:18 note "implicit instantiation first required here"
namespace n {
template <class T> struct Y { };
template <class T> int g(T) { return 0; }
template <class T> struct Z {
  template <class U> struct M { };
  template <class U> static int f(U) { return 0; }
  template <class U> static int v;
};
template <class T> template <class U> int Z<T>::v = 0;
}
n::Z<char>::M<int> m;
template <> template <> struct n::Z<char>::M<int> { };
int d = n::Z<char>::f(1);
template <> template <> int n::Z<char>::f(int) { return 1; }
int e = n::Z<char>::v<int>;
template <> template <> int n::Z<char>::v<int> = 2;
n::Y<int> y;
template <> struct n::Y<int> { };
int a = n::g(1);
template <> int n::g(int) { return 1; }
template <class T> struct P { template <class U> struct M; };
template <class T> template <class U> struct P<T>::M<U *> { };
P<int>::M<int *> pm;
template <> template <> struct P<int>::M<int *> { };
