// [clang ext] Where a calling convention reaches a function type with
// Swift parameter ABIs. The convention written in the decl-specifiers or
// after the declarator goes to the function type nearest the name, through
// any pointers, references, member pointers and arrays (clang
// `distributeFunctionTypeAttr`), before its parameter ABIs are checked
// (`checkExtParameterInfos`). One written after a `*`, a `&` or a grouping
// `(` applies to the type there, after the function type was built, so its
// `swift_context` / `swift_indirect_result` / `swift_error_result`
// parameters are checked without it and rejected. A parameter attribute
// after a `&` appertains to the parameter. GNU `noreturn` also applies
// through several pointers and arrays. spec/sema.md §4.5, WP-1827.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 34:88 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 35:83 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 36:89 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 38:91 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 39:86 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 40:79 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 41:79 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 42:80 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 43:80 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 44:88 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 45:88 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 47:86 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 48:84 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 49:96 error "'swift_indirect_result' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 49:96 error "'swift_indirect_result' parameters must be first parameters of function"
//@ diag: 50:91 error "'swift_error_result' parameter can only be used with swiftcall calling conventions"
//@ diag: 50:91 error "'swift_error_result' parameter must follow 'swift_context' parameter"
//@ diag: 51:81 error "'swift_context' parameter can only be used with swiftcall or swiftasynccall calling convention"
//@ diag: 52:53 error "'swift_indirect_result' parameter can only be used with swiftcall or swiftasynccall calling convention"
typedef int (__attribute__((swiftcall)) *fp1)(int, void *__attribute__((swift_context)));
typedef int (__attribute__((swiftcall)) *fp2)(void *__attribute__((swift_context)));
using fp3 = int (__attribute__((swiftcall)) *)(int, void *__attribute__((swift_context)));
struct S;
typedef int (__attribute__((swiftcall)) S::*mp1)(int, void *__attribute__((swift_context)));
void g1(int (__attribute__((swiftcall)) *p)(int, void *__attribute__((swift_context))));
int (__attribute__((swiftcall)) *v1)(int, void *__attribute__((swift_context)));
int (*__attribute__((swiftcall)) v2)(int, void *__attribute__((swift_context)));
int (__attribute__((swiftcall)) **v3)(int, void *__attribute__((swift_context)));
int (*__attribute__((swiftcall)) *v4)(int, void *__attribute__((swift_context)));
auto v5 = (int (__attribute__((swiftcall)) *)(int, void *__attribute__((swift_context))))0;
using fp4 = int (*__attribute__((swiftcall)))(int, void *__attribute__((swift_context)));
template <class T>
using A1 = int (__attribute__((swiftcall)) *)(T, void *__attribute__((swift_context)));
int (__attribute__((swiftcall)) *arr1[2])(int, void *__attribute__((swift_context)));
typedef void (__attribute__((swiftcall)) *fp5)(int, int &__attribute__((swift_indirect_result)));
typedef void (__attribute__((swiftcall)) *fp6)(void *__attribute__((swift_error_result)) *);
__attribute__((swiftcall)) int (*f1())(int, void *__attribute__((swift_context)));
void g2(int &__attribute__((swift_indirect_result)) x);

// Accepted: the convention is known when the function type is built.
typedef int __attribute__((swiftcall)) (*ok1)(int, void *__attribute__((swift_context)));
typedef int (*ok2)(int, void *__attribute__((swift_context))) __attribute__((swiftcall));
typedef int ok3(int, void *__attribute__((swift_context))) __attribute__((swiftcall));
typedef int (S::*ok4)(int, void *__attribute__((swift_context))) __attribute__((swiftcall));
void ok5(int (*p)(int, void *__attribute__((swift_context))) __attribute__((swiftcall)));
__attribute__((swiftcall)) int (*ok6)(int, void *__attribute__((swift_context)));
__attribute__((swiftcall)) int (**ok7)(int, void *__attribute__((swift_context)));
__attribute__((swiftcall)) int (*ok8[2])(int, void *__attribute__((swift_context)));
using ok9 = __attribute__((swiftcall)) int (*)(int, void *__attribute__((swift_context)));
void (__attribute__((swiftcall)) *ok10)(int, void *__attribute__((swift_async_context)));
__attribute__((swiftcall)) int (*ok11(void *__attribute__((swift_context))))(int);
__attribute__((swiftcall)) void ok12(int *__attribute__((swift_indirect_result)), int &__attribute__((swift_indirect_result)));
__attribute__((noreturn)) void (**nr1)();
void (__attribute__((noreturn)) **nr2)();
void (__attribute__((noreturn)) *nr3[2])();
__attribute__((noreturn)) void (*nr4[2])();
