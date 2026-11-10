//@ args: -O1 -Xclang -disable-llvm-passes
//@ module: mod_vtable_o1=Inputs/mod_vtable_o1.cppm
// [module.interface], [class.virtual], spec/modules.md §7 (WP-1198): at
// -O1 the vtable of a class attached to an imported module is not copied
// `available_externally` (clang `canSpeculativelyEmitVTable` →
// `shouldEmitInExternalSource`); it stays an `external` declaration, and
// A's complete-object destructor is a declaration of `_ZNW13mod_vtable_o11AD1Ev`.
import mod_vtable_o1;
int use() {
  A a;
  return a.f();
}
