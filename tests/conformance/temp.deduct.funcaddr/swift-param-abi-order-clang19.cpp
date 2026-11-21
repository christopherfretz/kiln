// [temp.deduct.funcaddr] [temp.deduct.decl] [temp.deduct.general]/8 A
// declaration's or an address's match whose specialization fails only on
// its Swift parameter ABIs: clang 19 forms the (invalid) declaration and
// reports the substitution failure with its reason, then, deduced again,
// without one; clang 22 compares the types first (the sibling -clang22
// test). Explicit arguments are checked against each parameter's ABI
// before deduction. Reduced from real code. spec/sema-templates.md §5,
// WP-1857.
//@ expect: compile-fail
//@ args: --target=x86_64-linux-gnu
//@ requires: clang<22
//@ strict-diags
//@ diag: 42:15 error "explicit instantiation of 'g3' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 41:55 note "candidate template ignored: substitution failure [with T = <>]: 'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 43:50 error "address of overloaded function 'g3' does not match required type 'void (void **) __attribute__((swiftcall))'"
//@ diag: 41:55 note "candidate template ignored: substitution failure [with T = <>]"
//@ diag: 44:50 error "address of overloaded function 'g3' does not match required type 'void (void **) __attribute__((swiftcall))'"
//@ diag: 41:55 note "candidate template ignored: substitution failure [with T = <>]"
//@ diag: 46:15 error "explicit instantiation of 'g7' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 45:64 note "candidate template ignored: substitution failure [with T = int, U = <>]: 'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 48:45 error "no function template matches function template specialization 'g8'"
//@ diag: 47:55 note "candidate template ignored: substitution failure [with T = <>]: 'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 49:106 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 50:15 error "explicit instantiation of 'g9' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 49:55 note "candidate template ignored: substitution failure [with T = <int *>]: 'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 53:42 error "explicit instantiation of 'h' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 52:52 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 54:46 error "address of overloaded function 'h' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 52:52 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 56:42 error "explicit instantiation of 'h2' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 55:52 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 59:13 error "no matching function for call to 'k'"
//@ diag: 58:51 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 59:19 error "no matching function for call to 'k'"
//@ diag: 58:51 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 60:47 error "address of overloaded function 'k' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 58:51 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 61:48 error "address of overloaded function 'k' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 58:51 note "candidate template ignored: substitution failure [with T = int]"
// A pack that leaves swift_error_result without its swift_context.
template <class... T> __attribute__((swiftcall)) void g3(T __attribute__((swift_context))..., void** __attribute__((swift_error_result)));
template void g3<>(void**);
__attribute__((swiftcall)) void (*p3)(void**) = &g3<>;
__attribute__((swiftcall)) void (*p4)(void**) = &g3;
template <class T, class... U> __attribute__((swiftcall)) void g7(T, U __attribute__((swift_context))..., void** __attribute__((swift_error_result)));
template void g7<int>(int, void**);
template <class... T> __attribute__((swiftcall)) void g8(T __attribute__((swift_context))..., void** __attribute__((swift_error_result)));
template <> __attribute__((swiftcall)) void g8<>(void**);
template <class... T> __attribute__((swiftcall)) void g9(int, T __attribute__((swift_indirect_result))...);
template void g9(int, int*);
// A swift_context parameter of non-pointer type.
template <class T> __attribute__((swiftcall)) void h(T __attribute__((swift_context)));
template __attribute__((swiftcall)) void h(int);
__attribute__((swiftcall)) void (*p)(int) = &h;
template <class T> __attribute__((swiftcall)) void h2(T __attribute__((swift_context)), int);
template __attribute__((swiftcall)) void h2<int>(int, int);
// A specialization formed (invalid) by a call is not formed again.
template<class T> __attribute__((swiftcall)) void k(T __attribute__((swift_context)));
void uk() { k(1); k(1); }
__attribute__((swiftcall)) void (*rk)(int) = &k<int>;
__attribute__((swiftcall)) void (*rk2)(int) = &k;
