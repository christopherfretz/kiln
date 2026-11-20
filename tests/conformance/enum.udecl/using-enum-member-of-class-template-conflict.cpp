// [enum.udecl]/3, [namespace.udecl]/8: a using-enum-declaration in a class
// template whose enumerator conflicts with a member is diagnosed once, in
// the template definition, not again when the class is instantiated.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:14 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 10:18 note "target of using declaration"
//@ diag: 12:7 note "conflicting declaration"
enum class Pig { OINK, SNORT };
template <int I> struct Hog {
  int OINK;
  using enum Pig;
};
Hog<1> h;
Hog<2> h2;
