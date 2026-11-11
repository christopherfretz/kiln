// [temp.deduct.general]/8, [temp.dep.expr], [temp.inst], [dcl.mptr],
// [over.match.class.deduct], [module.interface], spec/modules.md §11.11
// "Dependent types" (WP-1284): an importer's errors against dependent
// types and names from the declaration image are the ones against the
// replayed source: substitution failures in a dependent return type,
// errors inside an instantiated pattern (noted at the module's source),
// and a failed CTAD listing the module's guides (clang 19 names it unqualified).
//@ requires: clang>=22
//@ expect: compile-fail
//@ std: c++20
//@ module: imgd=Inputs/img-deptypes.cppm
//@ strict-diags
//@ diag: 21:3 error "no matching function for call to 'get'"
//@ diag: 22:3 error "no matching function for call to 'ptr'"
//@ diag: 23:16 error "cannot initialize a variable of type 'int (id::S::*)()' with an rvalue of type 'int id::S::*'"
//@ diag: 24:12 error "no viable constructor or deduction guide for deduction of template arguments of 'id::Wrap'"
import imgd;
struct B { static constexpr int value = 0; };
struct C {};
int main() {
  id::get(C{});
  id::ptr(C{});
  int (id::S::*bad)() = &id::S::m;
  id::Wrap w(1.0, 2);
  (void)id::sz<B>();
  (void)id::val<C>();
}
