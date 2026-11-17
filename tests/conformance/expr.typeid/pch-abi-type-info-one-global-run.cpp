// [expr.typeid], Itanium C++ ABI §2.9.5: with the runtime's `type_info`
// classes restored from a precompiled header, the reference D0's RTTI
// makes by symbol to `__si_class_type_info`'s vtable (emitted before the
// class's key functions) and the class's own vtable are one global: the
// descriptor's vtable pointer is the address point a constructed
// `__si_class_type_info` gets (WP-1620). The program reads pointers only:
// no exception or dynamic_cast runs through these classes.
//@ expect: run
//@ pch: Inputs/cxxabi_type_info_classes.h
B0::~B0() {}
D0::~D0() {}
const std::type_info *early() { return &typeid(D0); }
namespace __cxxabiv1 {
__shim_type_info::~__shim_type_info() {}
__class_type_info::~__class_type_info() {}
__si_class_type_info::~__si_class_type_info() {}
} // namespace __cxxabiv1
int main() {
  __cxxabiv1::__si_class_type_info si;
  void *const *rtti_vptr = reinterpret_cast<void *const *>(early());
  void *const *own_vptr = reinterpret_cast<void *const *>(&si);
  return *rtti_vptr == *own_vptr ? 0 : 1;
}
