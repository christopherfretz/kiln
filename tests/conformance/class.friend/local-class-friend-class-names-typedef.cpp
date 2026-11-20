// [class.friend]/11, [dcl.type.elab]/2: `friend struct Z;` in a local
// class finds the block's typedef `Z` as its prior declaration, and an
// elaborated-type-specifier naming a typedef-name is ill-formed, in a
// function and in a function template. kiln looks only for a class there
// and accepts both.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): WP-1734 follow-up, a local class's friend class lookup in the block scope ignores a typedef of that name
//@ diag: 17:28 error "typedef 'Z' cannot be referenced with the 'struct' specifier"
//@ diag: 16:15 note "declared here"
//@ diag: 22:28 error "typedef 'Z' cannot be referenced with the 'struct' specifier"
//@ diag: 21:15 note "declared here"

int f() {
  typedef int Z;
  struct L { friend struct Z; int v = 1; };
  return L().v;
}
template <class T> int g() {
  typedef int Z;
  struct L { friend struct Z; int v = 1; };
  return L().v;
}
int a = g<int>();
