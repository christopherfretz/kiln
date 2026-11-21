// [clang ext] The Swift parameter ABIs of a function type are checked again
// after substitution (clang `BuildFunctionType` → `checkExtParameterInfos`,
// at the type's begin), and so are the types of the parameters written in
// a function type inside another declarator (clang `SubstParmVarDecl`, at
// the attribute): a member alias, typedef or data member of a class
// template, a function template's own type (a substitution failure) and
// its trailing return type. clang 19 reports a class member's error twice.
// Reduced from real code. spec/sema.md §4.5, WP-1839.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 29:76 error "'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 30:1 note "in instantiation of template class 'S1<int>' requested here"
//@ diag: 32:83 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 32:42 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 33:1 note "in instantiation of template class 'S2<void *>' requested here"
//@ diag: 36:40 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 43:6 note "in instantiation of template class 'S3<>' requested here"
//@ diag: 38:38 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 40:30 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 48:13 error "no matching function for call to 'g3'"
//@ diag: 46:33 note "candidate template ignored: substitution failure [with T = <>]: 'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 50:98 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 51:13 error "no matching function for call to 't1'"
//@ diag: 50:6 note "candidate template ignored: substitution failure [with T = <int *>]: 'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 52:92 error "'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 53:9 note "in instantiation of template class 'S6<int>' requested here"
template <class T>
struct S1 { using F = __attribute__((swiftcall)) void (*)(T __attribute__((swift_context))); };
S1<int>::F x1;
S1<void *>::F y1;
template <class T> struct S2 { using F = void (*)(T __attribute__((swift_context))); };
S2<void *>::F x2;
template <class... T>
struct S3 {
  using F = __attribute__((swiftcall)) void (*)(T __attribute__((swift_context))...,
                                                void **__attribute__((swift_error_result)));
  typedef __attribute__((swiftcall)) void (*G)(T __attribute__((swift_context))...,
                                               void **__attribute__((swift_error_result)));
  __attribute__((swiftcall)) void (*f)(T __attribute__((swift_context))...,
                                       void **__attribute__((swift_error_result)));
};
S3<> x3;
S3<void *> y3;
template <class... T>
__attribute__((swiftcall)) void g3(T __attribute__((swift_context))...,
                                   void **__attribute__((swift_error_result)));
void v3() { g3<>(nullptr); }
template <class... T>
auto t1() -> __attribute__((swiftcall)) void (*)(int, T __attribute__((swift_indirect_result))...);
void v4() { t1<int *>(); }
template <class T> struct S6 { void f(__attribute__((swiftcall)) void (*)(T __attribute__((swift_context)))); };
S6<int> x6;
