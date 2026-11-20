// [enum.udecl], [basic.scope.scope]: a second
// `using enum Cow;` in one class scope redeclares its enumerators there,
// which a member-declaration may not do; clang and GCC reject it, in a
// class and in a class template's definition. kiln accepts it.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): WP-1734 follow-up, a repeated class-scope using-enum-declaration is not diagnosed
//@ diag: 17:3 error "redeclaration of using-enum declaration"
//@ diag: 16:14 note "previous using-enum declaration"
//@ diag: 21:3 error "redeclaration of using-enum declaration"
//@ diag: 20:14 note "previous using-enum declaration"

enum class Cow { MOO };
struct S {
  using enum Cow;
  using enum Cow;
};
template <class T> struct U {
  using enum Cow;
  using enum Cow;
};
U<int> u;
