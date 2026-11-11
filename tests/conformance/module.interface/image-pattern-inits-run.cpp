// [class.base.init]/2-9, [temp.inst]/4, [temp.variadic]/5,
// [dcl.attr.grammar]/5, [module.interface], spec/modules.md §11.11
// "Pattern mem-initializers" (WP-1310): the mem-initializers of a
// module's class template constructors (members, a dependent base named
// through a typedef, braced lists, delegation, a pack expansion, a
// constructor template, an out-of-line definition) are substituted by an
// importer's instantiations, and attributed locals are kept; under
// `--bmi-image` they come from the declaration image and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgpi=Inputs/img-pattern-inits.cppm
//@ stdout: 4 1 5 3 2 7 10 11 6 7
import imgpi;
extern "C" int printf(const char *, ...);
int main() {
  int x = 4;
  pi::Iter<int> a(&x);
  pi::Iter<const int> b(a);
  pi::Iter<int> z;
  pi::Derived<int> d(5);
  pi::Derived<long> e;
  pi::Multi<pi::B1, pi::B2> m(10);
  pi::Out<int> o(3);
  printf("%d %d %d %d %d %d %d %d %d %d\n", *b.p, z.p == nullptr, d.b, d.k, d.arr[1], (int)e.b,
         m.a, m.c, o.v, pi::local_attrs());
  return 0;
}
