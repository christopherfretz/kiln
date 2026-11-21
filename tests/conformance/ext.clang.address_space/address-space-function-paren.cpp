//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): WP-1803 follow-up: a space written inside grouping parentheses on a function type is not diagnosed
//@ diag: 8:22 error "function type may not be qualified with an address space"
// An address space applies to the pointer's pointee here, a function
// type (clang HandleAddressSpaceTypeAttribute).

void (__attribute__((address_space(1))) *fp)();
