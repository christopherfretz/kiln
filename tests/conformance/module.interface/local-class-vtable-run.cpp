// [class.local], [class.virtual], [module.interface], Itanium C++ ABI
// §5.2.3: the vtables of the dynamic local classes of an exported inline
// function are the module unit's even when nothing there uses them (one
// with a key function, one without); the importer's copy of the function
// constructs the objects and the program links (spec/modules.md §7).
//@ expect: run
//@ std: c++26
//@ module: lcvt=Inputs/local-class-vtable.cppm
//@ skip(clang): clang 22 and 19 define these vtables only where the module unit itself uses them, so the importer's constructors name undefined vtables at link time
import lcvt;

int main() { return lcvt_heap(4) == 12 ? 0 : 1; }
