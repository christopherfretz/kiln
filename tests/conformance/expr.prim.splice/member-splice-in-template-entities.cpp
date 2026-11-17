// [expr.ref]/7, [expr.prim.splice], [temp.dep.expr] (P2996R13): a member
// splice in a function template designates its member in every
// instantiation, with a non-dependent and a dependent operand, through
// `.` and `->`: a base member hidden in the derived class, one function of
// an overload set (no overload resolution), a hidden base member function
// (called on the base subobject), a static data member and static member
// function, a member function template with `template`, a bit-field, a
// member of an anonymous union, and a private member (a splice is not
// access-checked). kiln rebuilt the access by name lookup at
// instantiation. WP-1613.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: run
#include <meta>
struct B { int m = 1; int h = 10; int f(int) { return 100; } };
struct D : B {
  int h = 20;                       // hides B::h
  int f(int) { return 200; }        // hides B::f
  int f(long) { return 300; }
  static int s;
  static int sf() { return 7; }
  template <class T> int t() { return sizeof(T); }
  unsigned bf : 3 = 5;
  union { int u; float uf; };
private:
  int p = 42;
  friend struct Peek;
};
int D::s = 9;
consteval std::meta::info mem(std::meta::info c, std::string_view n) {
  for (auto m : members_of(c, std::meta::access_context::unchecked()))
    if (has_identifier(m) && identifier_of(m) == n) return m;
  throw 0;
}
consteval std::meta::info fn(std::meta::info c, std::string_view n, std::meta::info p) {
  for (auto m : members_of(c, std::meta::access_context::unchecked()))
    if (is_function(m) && has_identifier(m) && identifier_of(m) == n &&
        type_of(parameters_of(m)[0]) == p) return m;
  throw 0;
}
// Non-dependent operands in a template.
template <class T> int nd(T) {
  D d; d.u = 3; D *pd = &d;
  int r = 0;
  r += d.[:^^B::h:];                         // 10: the hidden base member
  r += pd->[:^^D::h:];                       // 20
  r += d.[:fn(^^B, "f", ^^int):](0);         // 100
  r += pd->[:fn(^^D, "f", ^^long):](0);      // 300: no overload resolution
  r += d.[:^^D::s:] + d.[:^^D::sf:]();       // 16
  r += d.template [:^^D::t:]<char>();        // 1
  r += d.[:^^D::bf:];                        // 5
  r += d.[:mem(^^D, "p"):];                  // 42: no access check
  r += d.[:^^D::u:];                         // 3
  return r;
}
// Dependent operands.
template <class C, std::meta::info H> int dep(C &c) {
  C *pc = &c;
  return c.[:H:] + pc->[:fn(^^C, "f", ^^int):](0) + c.template [:mem(^^C, "t"):]<int>() +
         pc->[:mem(^^C, "bf"):] + c.[:mem(^^C, "p"):];
}
int main() {
  if (nd(0) != 10 + 20 + 100 + 300 + 16 + 1 + 5 + 42 + 3) return 1;
  D d;
  if (dep<D, ^^B::h>(d) != 10 + 200 + 4 + 5 + 42) return 2;
  if (dep<D, ^^D::h>(d) != 20 + 200 + 4 + 5 + 42) return 3;
  return 0;
}
