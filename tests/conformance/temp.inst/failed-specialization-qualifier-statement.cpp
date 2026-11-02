// [temp.inst]/2, [stmt.ambig]: a statement `Q::name ident;` whose class
// template specialization qualifier `Q` is undefined or fails to
// instantiate reports only that error; the statement is skipped and
// declares nothing (clang `ClassifyName` -> `NC_Error`; libc++
// `tuple_element.verify`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:52 error "static assertion failed due to requirement '2 < 2': oob"
//@ diag: 23:3 note "in instantiation of template class 'Z<2, int>' requested here"
//@ diag: 24:3 error "use of undeclared identifier 'b'"
//@ diag: 25:3 error "implicit instantiation of undefined template 'Y<2, int>'"
//@ diag: 19:34 note "template is declared here"
//@ diag: 26:3 error "use of undeclared identifier 'c'"
//@ diag: 21:61 error "static assertion failed due to requirement '2 < 2': oobw"
//@ diag: 28:3 note "in instantiation of template class 'Z<2, W<short>>' requested here"
//@ diag: 18:52 error "static assertion failed due to requirement '3 < 2': oob"
//@ diag: 29:3 note "in instantiation of template class 'Z<3, int>' requested here"
template <int I, class T> struct Z { static_assert(I < 2, "oob"); };
template <int I, class T> struct Y;
template <class T> struct W {};
template <int I, class T> struct Z<I, W<T>> { static_assert(I < 2, "oobw"); };
void f() {
  Z<2, int>::type b;
  b = 1;
  Y<2, int>::type c;
  c = 1;
  typedef W<short> T;
  Z<2, T>::type d;
  Z<3, int>::v + 1;
}
