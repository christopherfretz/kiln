// [clang ext] The Swift parameter ABIs of an alias template's function
// type are checked again when the alias is instantiated (clang
// `BuildFunctionType` → `checkExtParameterInfos`, at the alias
// declaration): a pack expansion can move a `swift_indirect_result` away
// from the front, or leave a `swift_error_result` without the
// `swift_context` before it. The parameter types are not checked again
// (`A1<int>` is accepted, as in clang). A pattern that is wrong whatever
// the arguments is reported at its definition. clang 19 reports each
// instantiation's error twice. spec/sema.md §4.5, WP-1810.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 28:1 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 31:1 note "in instantiation of template type alias 'P2' requested here"
//@ diag: 38:96 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 38:1 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 40:1 note "in instantiation of template type alias 'P4' requested here"
//@ diag: 43:71 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
template <class T>
using A1 = __attribute__((swiftcall)) int (*)(int, T __attribute__((swift_context)));
A1<void *> a1;
A1<int> a2;
template <class... T>
using P1 = __attribute__((swiftcall)) void (*)(T __attribute__((swift_indirect_result))..., int);
P1<int *, int *> p1a;
P1<> p1b;
template <class... T>
using P2 = __attribute__((swiftcall)) void (*)(T __attribute__((swift_context))...,
                                               void **__attribute__((swift_error_result)));
P2<void *> p2a;
P2<> p2b;
template <class... T>
using P3 = __attribute__((swiftcall)) void (*)(int *__attribute__((swift_indirect_result)),
                                               T __attribute__((swift_indirect_result))...);
P3<> p3a;
P3<int *> p3b;
template <class... T>
using P4 = __attribute__((swiftcall)) void (*)(int, T __attribute__((swift_indirect_result))...);
P4<> p4a;
P4<int *> p4b;
template <class T> using A2 = int (*)(int, T __attribute__((swift_async_context)));
A2<int> a3;
template <class T> using A3 = int (*)(T __attribute__((swift_context)));
