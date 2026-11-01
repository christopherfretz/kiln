// [class.mem.general]/8, [except.spec]/8: the implicit exception
// specification of a nested class's default constructor needs its default
// member initializers, which are parsed at the end of the outermost class.
// Needing it earlier is diagnosed once per field, under one "evaluation of
// exception specification" context note at the use; the fields are then
// invalid, so the constructor's definition, its constant evaluation and the
// enclosing class's default constructor do not cascade.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:25 error "default member initializer for 'a' needed within definition of enclosing class 'V' outside of member functions"
//@ diag: 26:22 note "in evaluation of exception specification for 'V::X::X' needed here"
//@ diag: 25:18 note "default member initializer declared here"
//@ diag: 30:10 error "default member initializer for 'a' needed within definition of enclosing class 'U' outside of member functions"
//@ diag: 31:28 note "in evaluation of exception specification for 'U::X::X' needed here"
//@ diag: 30:18 note "default member initializer declared here"
//@ diag: 37:10 error "default member initializer for 'a' needed within definition of enclosing class 'A' outside of member functions"
//@ diag: 38:38 note "in evaluation of exception specification for 'A::X::X' needed here"
//@ diag: 37:18 note "default member initializer declared here"
//@ diag: 43:10 error "default member initializer for 'a' needed within definition of enclosing class 'D' outside of member functions"
//@ diag: 44:28 note "in evaluation of exception specification for 'D::Y::Y' needed here"
//@ diag: 43:18 note "default member initializer declared here"
//@ diag: 43:10 error "default member initializer for 'b' needed within definition of enclosing class 'D' outside of member functions"
//@ diag: 43:29 note "default member initializer declared here"
struct V {
  struct X { int a = 2; X() = default; };
  static constexpr X x{};
  static constexpr int q = X().a;
};
struct U {
  struct X { int a = 2; };
  static constexpr int q = X().a;
  static constexpr int r = X{}.a;
  X m;
};
U u;
struct A {
  struct X { int a = 2; };
  static constexpr bool b = noexcept(X());
  static constexpr bool c = __is_nothrow_constructible(X);
  X x;
};
struct D {
  struct Y { int a = 1; int b = 2; };
  static constexpr int r = Y().b;
  static constexpr int s = Y().a;
};
template<class T> struct Outer {
  struct X { T a = T(); X() = default; };
  static constexpr int q = X().a;
};
Outer<int> o;
int main() { return Outer<int>::X().a + A().x.a; }
