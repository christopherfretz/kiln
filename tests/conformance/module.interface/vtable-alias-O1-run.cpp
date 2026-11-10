// [module.interface], [class.virtual], [class.dtor], spec/codegen.md
// (vtables): at -O1 an importer may emit an imported vtable as
// `available_externally`, and its destructor slots name the destructor's
// variants. kiln rejects the importer instead (kind 1 and kind 2 alike)
// when the import precedes `main` on another line.
//@ expect: run
//@ std: c++20
//@ args: -O1
//@ module: vta=Inputs/vtable-alias.cppm
//@ exit: 4
//@ xfail(kiln): WP-1120 follow-up, an available_externally vtable references a structor alias not emitted yet (kiln_codegen/src/vtable.rs:852)
import vta;

int main() {
  A a;
  return a.f();
}
