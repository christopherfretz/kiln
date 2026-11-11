// [temp.constr.constr], [temp.constr.order], [temp.concept],
// [expr.prim.req], [temp.param]/4, [dcl.decl.general]/4,
// [module.interface], spec/modules.md §11.11 "Constraints" (WP-1284):
// concepts, constrained template parameters, requires-clauses (template
// heads and trailing) and requires-expressions exported from a module;
// under `--bmi-image` they come from the declaration image, satisfaction
// is checked by the importer's instantiations and the program behaves
// exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgc=Inputs/img-constraints.cppm
//@ stdout: 1 2 3 4 2 5 6 7 1 17 0 0 8 1 1 1
import imgc;
extern "C" int printf(const char *, ...);
static_assert(ic::Small<char> && !ic::Small<int> && ic::Big<int>);
struct A { using type = int; int foo() { return 7; } void bar() noexcept {} char c[2]; };
struct B { int foo() { return 8; } };
struct C { using type = int; int foo() { return 9; } void bar() {} char c[2]; };
int main() {
  ic::BigAlias<int> x = 7;
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", ic::pick('a'), ic::pick(1),
         ic::pick2(1L), ic::pick2(1), ic::count('a', (short)1), ic::Box<char>().h(),
         ic::Box<int>().h(), x, ic::vt<char>, ic::g(A{}), ic::g(B{}), ic::g(C{}), ic::f(B{}),
         ic::Addable<int> && !ic::Addable<A>, ic::has_foo<A> && !ic::has_foo<int>,
         ic::Packy<int, long>);
  return 0;
}
