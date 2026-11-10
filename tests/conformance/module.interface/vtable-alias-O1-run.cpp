// [module.interface], [class.virtual], [class.dtor], spec/codegen.md
// (vtables): at -O1 an importer may emit an imported vtable as
// `available_externally`, but not that of a class attached to the
// imported module, whose unit alone emits it (clang
// `shouldEmitInExternalSource`): the vtable stays an `external`
// declaration, and A's D1 a declaration. kiln used to copy it and fail on
// its destructor slots ("a vtable references a structor alias defined
// later", WP-1198).
//@ expect: run
//@ std: c++20
//@ args: -O1
//@ module: vta=Inputs/vtable-alias.cppm
//@ exit: 4
import vta;

int main() {
  A a;
  return a.f();
}
