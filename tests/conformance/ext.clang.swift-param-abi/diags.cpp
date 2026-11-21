// The Swift parameter ABI checks (clang `SemaSwift::AddParameterABIAttr`
// at the parameter, `checkExtParameterInfos` on the function type): a
// pointer (`swift_error_result`: a pointer or reference to an unqualified
// pointer) parameter, `swift_indirect_result` parameters first,
// `swift_error_result` right after `swift_context`, `swiftcall` (or
// `swiftasynccall`, except for `swift_error_result`) for all but
// `swift_async_context`, no two different ABIs on one parameter; on
// anything but a parameter the attribute is ignored. The ABIs are part of
// the function type: a redeclaration differing in them conflicts, and a
// pointer to one function type does not convert to the other. In a
// template the type check is in the immediate context. spec/sema.md
// §4.5, WP-1745.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 50:9 error "conflicting types for 'h'"
//@ diag: 49:9 note "previous declaration is here"
//@ diag: 53:3 error "cannot initialize a variable of type 'P' (aka 'void (*)(void *) __attribute__((swiftcall))') with an lvalue of type 'void (__attribute__((swift_context)) void *) __attribute__((swiftcall))'"
//@ diag: 54:23 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 55:25 error "'swift_error_result' parameter must have pointer to unqualified pointer type; type here is 'void *'"
//@ diag: 47:28 note "expanded from macro 'ERR'"
//@ diag: 56:12 error "'swift_context' parameter must have pointer type; type here is 'int'"
//@ diag: 45:28 note "expanded from macro 'CTX'"
//@ diag: 57:12 error "'swift_indirect_result' parameter must have pointer type; type here is 'int'"
//@ diag: 48:28 note "expanded from macro 'IND'"
//@ diag: 58:29 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 59:19 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 60:36 error "'swift_error_result' parameter can only be used with swiftcall calling conventions"
//@ diag: 61:17 error "swift_error_result and 'swift_context' attributes are not compatible"
//@ diag: 47:28 note "expanded from macro 'ERR'"
//@ diag: 61:13 note "conflicting attribute is here"
//@ diag: 45:28 note "expanded from macro 'CTX'"
//@ diag: 62:5 warning "'swift_context' attribute only applies to parameters"
//@ diag: 45:28 note "expanded from macro 'CTX'"
//@ diag: 63:26 error "'swift_error_result' parameter must have pointer to unqualified pointer type; type here is 'void *const *'"
//@ diag: 47:28 note "expanded from macro 'ERR'"
//@ diag: 65:14 error "no matching function for call to 'tt'"
//@ diag: 64:28 note "candidate template ignored: substitution failure [with T = int]: 'swift_indirect_result' parameter must have pointer type; type here is 'int'"
//@ diag: 66:28 error "'swift_context' attribute takes no arguments"
//@ diag: 67:52 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 68:20 error "'swift_indirect_result' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 69:35 error "'swift_indirect_result' parameters must be first parameters of function"
#define SC __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
#define CTX __attribute__((swift_context))
#define ACTX __attribute__((swift_async_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
SC void h(CTX void *);
SC void h(void *);
__attribute__((swiftcall)) void m(__attribute__((swift_context)) void *);
typedef void (*P)(void *) __attribute__((swiftcall));
P p = m;
SC void e1(ERR void **e);
SC void e2(CTX void *c, ERR void *e);
SC void e3(CTX int c);
SC void e4(IND int c);
SC void e5(int x, IND void *c);
void e6(CTX void *c);
SA void e7(CTX void *c, ERR void **e);
SC void e10(CTX ERR void **c);
int CTX gv;
SC void e11(CTX void *c, ERR void *const *e);
template <class T> SC void tt(IND T c) {}
void use() { tt(1); }
SC void e16(__attribute__((swift_context(1))) void *c);
SC void e17(CTX void *c, ERR void **e1, ERR void **e2);
void e18(IND void *);
SC void e19(CTX void *, IND void *);
