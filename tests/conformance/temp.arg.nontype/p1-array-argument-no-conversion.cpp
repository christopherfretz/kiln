// [temp.arg.nontype]/2, [expr.const]/12: an array argument converts only to
// a pointer parameter (or binds a reference, or initializes a class); for
// an integer parameter there is no implicit conversion, and to `bool` the
// boolean conversion is not allowed in a converted constant expression,
// also when the array's type was dependent where it was named. (clang 22
// adds the "template parameter is declared here" notes.)
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:5 error "value of type 'int[2]' is not implicitly convertible to 'int'"
//@ diag: 21:15 note "in instantiation of function template specialization"
//@ diag: 15:15 note "template parameter is declared here"
//@ diag: 23:4 error "conversion from 'int[3]' to 'bool' is not allowed in a converted constant expression"
//@ diag: 16:16 note "template parameter is declared here"
template <int N> struct A {};
template <bool B> struct Bo {};
template <class T> void f() {
  static T arr[2];
  A<arr> a;
}
template void f<int>();
int g[3];
Bo<g> b;
int main() {}
