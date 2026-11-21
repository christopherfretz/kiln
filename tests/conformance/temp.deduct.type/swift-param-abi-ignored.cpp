// [temp.deduct.type]/1 [temp.deduct.call]/4 [temp.deduct.funcaddr]
// [temp.deduct.decl] Template argument deduction ignores the Swift
// parameter ABIs (`swift_context` and the others) of a function type, as
// clang's `DeduceTemplateArgumentsByTypeMatch`; the deduced A then
// differs from the argument and is rejected afterwards: by the original
// argument check of a call, by the type match of an address or a
// declaration, and partial specializations that differ only in an ABI are
// distinct. Reduced from real code. spec/sema-templates.md §5, WP-1857.
//@ expect: compile-fail
//@ args: --target=x86_64-linux-gnu
//@ strict-diags
//@ diag: 32:3 error "no matching function for call to 'f1'"
//@ diag: 29:25 note "of 1st parameter does not match adjusted type 'A0' (aka 'void (*)(void *) __attribute__((swiftcall))') of argument [with T = void *]"
//@ diag: 35:3 error "no matching function for call to 'f2'"
//@ diag: 30:25 note "of 1st parameter does not match adjusted type 'A1' (aka 'void (*)(__attribute__((swift_context)) void *) __attribute__((swiftcall))') of argument [with T = void *]"
//@ diag: 41:12 error "address of overloaded function 'g1' does not match required type 'void (void *) __attribute__((swiftcall))'"
//@ diag: 38:52 note "candidate template ignored: could not match 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))' against 'void (void *) __attribute__((swiftcall))'"
//@ diag: 44:11 error "address of overloaded function 'g2' does not match required type 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))'"
//@ diag: 39:52 note "candidate template ignored: could not match 'void (void *) __attribute__((swiftcall))' against 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))'"
//@ diag: 45:12 error "address of overloaded function 'g1' does not match required type 'void (void *) __attribute__((swiftcall))'"
//@ diag: 38:52 note "candidate template ignored: could not match 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))' against 'void (void *) __attribute__((swiftcall))'"
//@ diag: 54:45 error "no function template matches function template specialization 'g1'"
//@ diag: 38:52 note "candidate template ignored: could not match 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))' against 'void (void *) __attribute__((swiftcall))'"
//@ diag: 55:42 error "explicit instantiation of 'g2' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 39:52 note "candidate template ignored: could not match 'void (void *) __attribute__((swiftcall))' against 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))'"
// A call: deduced, then the original argument check ([temp.deduct.call]/4).
using A0 = __attribute__((swiftcall)) void (*)(void *);
using A1 = __attribute__((swiftcall)) void (*)(void * __attribute__((swift_context)));
template <class T> void f1(__attribute__((swiftcall)) void (*)(T __attribute__((swift_context))));
template <class T> void f2(__attribute__((swiftcall)) void (*)(T));
void u1(A0 a0, A1 a1) {
  f1(a0);
  f1(a1);
  f2(a0);
  f2(a1);
}
// An address: deduced, then the function type match.
template <class T> __attribute__((swiftcall)) void g1(T __attribute__((swift_context)));
template <class T> __attribute__((swiftcall)) void g2(T);
void u2() {
  A0 p0 = &g1;
  A1 p1 = &g1;
  A0 q0 = &g2;
  A1 q1 = g2;
  A0 r0 = &g1<void*>;
}
// Partial specializations differing only in the ABI are distinct.
template <class T> struct S;
template <class T> struct S<__attribute__((swiftcall)) void(T __attribute__((swift_context)))> { static const int v = 1; };
template <class T> struct S<__attribute__((swiftcall)) void(T)> { static const int v = 2; };
static_assert(S<__attribute__((swiftcall)) void(void* __attribute__((swift_context)))>::v == 1);
static_assert(S<__attribute__((swiftcall)) void(void*)>::v == 2);
// Declarations: deduced, then the function type match.
template <> __attribute__((swiftcall)) void g1(void *);
template __attribute__((swiftcall)) void g2(void * __attribute__((swift_context)));
