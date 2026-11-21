//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:22 error "function type may not be qualified with an address space"
// An address space applies to the pointer's pointee here, a function
// type (clang HandleAddressSpaceTypeAttribute).

void (__attribute__((address_space(1))) *fp)();
