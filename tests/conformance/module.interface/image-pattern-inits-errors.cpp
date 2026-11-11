// [class.base.init]/7, [temp.inst]/4, [module.interface], spec/modules.md
// §11.11 "Pattern mem-initializers" (WP-1310): an importer's errors
// against the mem-initializers of a module's class template constructors
// built from the declaration image are the ones against the replayed
// source: overload resolution over the constructors, and a
// mem-initializer that does not instantiate (diagnosed in the module).
//@ expect: compile-fail
//@ std: c++20
//@ module: imgpi=Inputs/img-pattern-inits.cppm
//@ diag: 15:29 error "no matching constructor for initialization of 'pi::Multi<pi::B1, pi::B2>'"
//@ diag: 16:17 error "no matching constructor for initialization of 'pi::Iter<int>'"
import imgpi;
struct NC {};
void f() {
  pi::Multi<pi::B1, pi::B2> m;
  pi::Iter<int> i(1, 2);
  pi::Out<NC> o(NC{});
  pi::Derived<NC> d;
}
