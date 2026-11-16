// [expr.prim.splice]/2.7, /3, [temp.names]/3: a splice-expression that
// designates a template needs `template` (without it a following `<` is
// less-than, and a plain splice of a template is ill-formed); `template [:
// r :]` without template arguments must designate a function template,
// not a variable, a function, a specialization or a variable template,
// here or after substitution. The missing `template` is diagnosed as
// GCC 16.1 does, a warning in -Wmissing-template-keyword that
// -pedantic-errors makes an error (kiln recovers as if it were written).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 34:12 warning "missing 'template' keyword before a splice-specifier that designates a template"
//@ diag: 35:12 warning "missing 'template' keyword before a splice-specifier that designates a template"
//@ diag: 36:12 warning "missing 'template' keyword before a splice-specifier that designates a template"
//@ diag: 37:8 warning "missing 'template' keyword before a splice-specifier that designates a template"
//@ diag: 38:21 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 39:21 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 40:21 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 41:21 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 45:22 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 47:9 note "in instantiation of function template specialization 'call<^^v>' requested here"
template <typename T> constexpr int twice(T t) { return 2 * t; }
template <typename T> constexpr T forty = 40;
constexpr int v = 1;
int plain(int);
struct S {
  template <typename T> int get(T) const;
};

void f(S s) {
  (void)[: ^^twice :](1);
  (void)[: ^^twice :]<int>(1);
  (void)[: ^^forty :]<int>;
  s.[: ^^S::get :](1);
  (void)template [: ^^v :];
  (void)template [: ^^plain :](1);
  (void)template [: ^^twice<int> :](1);
  (void)template [: ^^forty :];
}

template <decltype(^^int) R> int call() {
  return template [: R :];
}
int x = call<^^v>();
