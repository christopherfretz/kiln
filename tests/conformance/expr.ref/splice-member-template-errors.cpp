// [expr.prim.splice]/3: in `x.template [: r :]` without a template
// argument list, r shall designate a function template; with one, a
// template. A non-template, a static data member template without
// arguments, and a template of an unrelated class are errors.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
//@ diag: 17:23 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 18:23 error "splice after 'template' without a template argument list does not designate a function template"
//@ diag: 19:23 error "splice in a member access position does not designate a member of 'const S'"
struct S { int m; template <int N> static constexpr int a = N; };
struct O { template <int N> static constexpr int b = N; };
constexpr S s{};
int f() {
  return s.template [:^^S::m:] +
         s.template [:^^S::a:] +
         s.template [:^^O::b:]<1>;
}
