// [dcl.attr.deprecated]/2, GNU `unavailable`: a use of a deprecated or
// unavailable conversion function warns (errors) where the conversion is
// applied, in a cast and in copy-initialization; an enumerator of a member
// enumeration of a class template specialization has its pattern's
// attribute. Found running GCC's C++ testsuite (clang diagnoses the same
// uses). WP-1552.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:9 warning "'operator int' is deprecated"
//@ diag: 23:33 note "'operator int' has been explicitly marked deprecated here"
//@ diag: 33:7 warning "'operator int' is deprecated"
//@ diag: 23:33 note "'operator int' has been explicitly marked deprecated here"
//@ diag: 34:9 error "'operator long' is unavailable: no"
//@ diag: 24:3 note "'operator long' has been explicitly marked unavailable here"
//@ diag: 35:8 error "'operator long' is unavailable: no"
//@ diag: 24:3 note "'operator long' has been explicitly marked unavailable here"
//@ diag: 37:25 warning "'A' is deprecated"
//@ diag: 28:30 note "'A' has been explicitly marked deprecated here"
//@ diag: 38:21 error "'B' is unavailable"
//@ diag: 28:44 note "'B' has been explicitly marked unavailable here"
struct Foo {
  operator int() __attribute__((deprecated));
  operator long() __attribute__((unavailable("no")));
  operator char();
};
template <typename> struct S {
  enum E1 { A __attribute__((deprecated)), B __attribute__((unavailable)), C };
};
void g() {
  Foo f;
  (void)(int)f;
  int i = f;
  (void)(long)f;
  long l = f;
  char c = f;
  auto a1 = S<int>::E1::A;
  auto b1 = S<int>::B;
  auto c1 = S<int>::C;
}
