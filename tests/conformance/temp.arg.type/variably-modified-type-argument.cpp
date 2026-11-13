// [temp.arg.type] (the variable length array extension): a variably
// modified type cannot be a template type argument (clang
// `err_variably_modified_template_arg`), written directly, through
// `decltype` of a variable length array, or as a pointer to one; a
// constant-size array type can. WP-1386.
//@ expect: compile-fail
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 14:5 error "variably modified type 'decltype(a)' (aka 'int[n]') cannot be used as a template argument"
//@ diag: 15:5 error "variably modified type 'int (*)[n]' cannot be used as a template argument"
template <class T> struct S {};
void f(int n) {
  int a[n];
  S<decltype(a)> s;
  S<int (*)[n]> t;
  S<int[4]> u;
  (void)a;
}
