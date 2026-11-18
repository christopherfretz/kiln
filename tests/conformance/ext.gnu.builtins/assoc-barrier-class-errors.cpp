// GCC's __builtin_assoc_barrier on a class operand is the operand's
// object, not a copy: initializing another object from it copies, and a
// deleted copy constructor is then an error (GCC 16.1 rejects the same
// line: "use of deleted function"). Operands that are neither a class nor
// a scalar or vector stay rejected (GCC also accepts void
// and std::nullptr_t operands; kiln does not). WP-1611.
//@ std: c++20
//@ expect: compile-fail
//@ skip(clang): clang 19/22 have no __builtin_assoc_barrier
//@ strict-diags
//@ diag: 20:12 error "call to deleted constructor of 'NoCopy'"
//@ diag: 17:5 note "'NoCopy' has been explicitly marked deleted here"
//@ diag: 21:29 error "1st argument must be a scalar or vector type (was 'void')"
//@ diag: 22:29 error "1st argument must be a scalar or vector type (was 'std::nullptr_t')"
struct NoCopy {
    NoCopy() = default;
    NoCopy(const NoCopy &) = delete;
};
void t(NoCopy &n) {
    NoCopy m = __builtin_assoc_barrier(n);
    __builtin_assoc_barrier((void)0);
    __builtin_assoc_barrier(nullptr);
}
