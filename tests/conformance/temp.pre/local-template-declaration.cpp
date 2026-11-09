// [temp.pre]/2: a template-declaration can appear only in namespace or
// class scope; one in a block is clang 22's "templates can only be
// declared in namespace or class scope" at `template`, and is dropped
// (WP-1063). Clang 19 gives "expected expression" (the -19 sibling).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 12:3 error "templates can only be declared in namespace or class scope"
//@ diag: 13:3 error "templates can only be declared in namespace or class scope"
//@ diag: 16:14 error "templates can only be declared in namespace or class scope"
void f() {
  template<class T> struct D { T t; };
  template<class T> T v = 0;
}
struct S {
  void g() { template<class T> struct Q {}; }
};
