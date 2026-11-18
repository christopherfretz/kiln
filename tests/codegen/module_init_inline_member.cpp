// An inline static data member with a dynamic initializer, defined in its
// class in a module interface, is one of the module's initializers for
// clang 22 (`addModuleInitializer`), so an importer calls `_ZGIW` even
// when it is the module's only one; its initialization itself stays the
// comdat-keyed llvm.global_ctors entry, in the module unit and in each
// importer (WP-1653). kiln emits the importer's copies at the import, for
// every such member; clang 22 for the referenced ones, after main
// (spec/modules.md §7: an `Abi` deviation, the order of the functions).
//@ module: mod_init_member=Inputs/mod_init_member.cppm
import mod_init_member;
int main() { return C::D::x + U::y + hidden_z(); }
