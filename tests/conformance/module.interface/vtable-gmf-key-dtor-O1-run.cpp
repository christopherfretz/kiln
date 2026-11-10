// [module.global.frag], [class.virtual], [class.dtor], spec/modules.md §7
// (WP-1198): at -O1 an importer copies the vtable of a global-module-
// fragment class whose key destructor the imported unit defines
// `available_externally`; its destructor slots name D1/D0 as
// declarations (the unit's object defines them), not as aliases this
// translation unit would define.
//@ expect: run
//@ std: c++20
//@ args: -O1
//@ module: vtg=Inputs/vtable-gmf-key-dtor.cppm
//@ exit: 5
import vtg;

int main() {
  G g;
  return g.f();
}
