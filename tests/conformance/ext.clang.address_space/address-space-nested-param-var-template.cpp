// [clang ext] A variable template's function type is checked again when the
// variable is instantiated: a parameter qualified with an address space,
// at the parameter, and the Swift parameter ABIs, at the declared type's
// begin (clang `SubstParmVarDecl` and `checkExtParameterInfos`). clang
// reports each twice (for the declaration and for the definition), so the
// list is not strict. Reduced from real code. WP-1823, WP-1839.
//@ expect: compile-fail
//@ diag: 13:66 error "parameter may not be qualified with an address space"
//@ diag: 14:19 note "in instantiation of variable template specialization 'vt' requested here"
//@ diag: 15:63 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 15:20 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 16:19 note "in instantiation of variable template specialization 'vs' requested here"
template <int N> void (*vt)(int __attribute__((address_space(N)))) = nullptr;
void uv() { (void)vt<1>; }
template <class T> void (*vs)(T __attribute__((swift_context))) = nullptr;
void us() { (void)vs<void *>; }
