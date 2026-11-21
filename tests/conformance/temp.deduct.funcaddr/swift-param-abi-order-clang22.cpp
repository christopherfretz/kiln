// [temp.deduct.funcaddr] [temp.deduct.decl] [temp.deduct.general]/8 A
// declaration's or an address's match whose specialization fails only on
// its Swift parameter ABIs, against a target type that the substituted
// type differs from: clang 22 compares the types before it instantiates
// the declaration ("could not match", nothing formed), clang 19 reports
// the substitution failure (the sibling -clang19 test). A specialization
// a call formed invalid is a substitution failure without a reason when
// deduced again, and explicit arguments are checked against each
// parameter's ABI before deduction (both releases). Reduced from real
// code. spec/sema-templates.md §5, WP-1857.
//@ expect: compile-fail
//@ args: --target=x86_64-linux-gnu
//@ requires: clang>=22
//@ strict-diags
//@ diag: 44:15 error "explicit instantiation of 'g3' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 43:55 note "candidate template ignored: could not match 'void (__attribute__((swift_error_result)) void **) __attribute__((swiftcall))' against 'void (void **) __attribute__((swiftcall))'"
//@ diag: 45:50 error "address of overloaded function 'g3' does not match required type 'void (void **) __attribute__((swiftcall))'"
//@ diag: 43:55 note "candidate template ignored: could not match 'void (__attribute__((swift_error_result)) void **) __attribute__((swiftcall))' against 'void (void **) __attribute__((swiftcall))'"
//@ diag: 46:50 error "address of overloaded function 'g3' does not match required type 'void (void **) __attribute__((swiftcall))'"
//@ diag: 43:55 note "candidate template ignored: could not match 'void (__attribute__((swift_error_result)) void **) __attribute__((swiftcall))' against 'void (void **) __attribute__((swiftcall))'"
//@ diag: 48:15 error "explicit instantiation of 'g7' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 47:64 note "candidate template ignored: could not match 'void (int, __attribute__((swift_error_result)) void **) __attribute__((swiftcall))' against 'void (int, void **) __attribute__((swiftcall))'"
//@ diag: 50:45 error "no function template matches function template specialization 'g8'"
//@ diag: 49:55 note "candidate template ignored: could not match 'void (__attribute__((swift_error_result)) void **) __attribute__((swiftcall))' against 'void (void **) __attribute__((swiftcall))'"
//@ diag: 51:106 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 52:15 error "explicit instantiation of 'g9' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 51:55 note "candidate template ignored: could not match 'void (int, __attribute__((swift_indirect_result)) int *) __attribute__((swiftcall))' against 'void (int, int *) __attribute__((swiftcall))'"
//@ diag: 55:42 error "explicit instantiation of 'h' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 54:52 note "candidate template ignored: could not match 'void (__attribute__((swift_context)) int) __attribute__((swiftcall))' against 'void (int) __attribute__((swiftcall))'"
//@ diag: 56:46 error "address of overloaded function 'h' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 54:52 note "candidate template ignored: could not match 'void (__attribute__((swift_context)) int) __attribute__((swiftcall))' against 'void (int) __attribute__((swiftcall))'"
//@ diag: 58:42 error "explicit instantiation of 'h2' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 57:52 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 61:13 error "no matching function for call to 'k'"
//@ diag: 60:51 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 61:19 error "no matching function for call to 'k'"
//@ diag: 60:51 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 62:47 error "address of overloaded function 'k' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 60:51 note "candidate template ignored: substitution failure [with T = int]: 'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 63:48 error "address of overloaded function 'k' does not match required type 'void (int) __attribute__((swiftcall))'"
//@ diag: 60:51 note "candidate template ignored: substitution failure [with T = int]"
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
