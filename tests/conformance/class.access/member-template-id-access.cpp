// [class.access]/4: the name of a member template in a qualified
// template-id (`C::f<int>(1)`, `C::v<int>`, `C<T>::template v<int>`) is
// access-checked like any other member name; kiln checked none of them.
// A friend keeps its access. Found while fixing the friend overload hole
// found running GCC's C++ testsuite (WP-1513).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:23 error "'h' is a private member of 'V'"
//@ diag: 18:33 note "implicitly declared private here"
//@ diag: 22:38 error "'y' is a private member of 'V'" dup(clang)
//@ diag: 19:43 note "implicitly declared private here"
//@ diag: 30:54 error "'h' is a private member of 'W<int>'"
//@ diag: 34:9 note "in instantiation of function template specialization 'dep<int>' requested here"
//@ diag: 26:33 note "implicitly declared private here"
//@ diag: 30:81 error "'y' is a private member of 'W<int>'"
//@ diag: 27:43 note "implicitly declared private here"
class V {
  template <class U> static int h(U) { return 0; }
  template <class U> static constexpr int y = 0;
  friend int fr();
};
int use() { return V::h<int>(1) + V::y<int>; }
int fr() { return V::h<int>(1) + V::y<int>; }

template <class T> class W {
  template <class U> static int h(U) { return 0; }
  template <class U> static constexpr int y = 0;
  friend struct F;
};
template <class T> int dep() { return W<T>::template h<int>(1) + W<T>::template y<int>; }
struct F {
  template <class T> static int ok() { return W<T>::template h<int>(1) + W<T>::template y<int>; }
};
int u = dep<int>() + F::ok<int>();
