// [expr.typeid], [module.interface], Itanium C++ ABI §2.9.5: in a module
// interface unit that defines `__si_class_type_info` (attached to the
// global module) after RTTI that names its vtable by symbol, the two are
// one global: D0's descriptor points at the class's own vtable (WP-1620).
// The program reads pointers only.
//@ expect: run
//@ std: c++20
//@ module: cxxabi_si=Inputs/cxxabi_si_module.cppm
import cxxabi_si;
int main() {
  return *reinterpret_cast<void *const *>(early()) == si_address_point() ? 0 : 1;
}
