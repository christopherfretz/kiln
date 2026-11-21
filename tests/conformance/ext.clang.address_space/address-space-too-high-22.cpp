// The largest address_space(N) is clang's MaxAddressSpace less its first
// target address space: 8388581 at clang 22 (WP-1803).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 9:16 error "address space is larger than the maximum supported (8388581)"
//@ diag: 10:16 error "address space is larger than the maximum supported (8388581)"

__attribute__((address_space(8388582))) int a;
__attribute__((address_space(1u << 31))) int b;
__attribute__((address_space(8388581))) int c;
