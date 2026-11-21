// The negative twin of diags.cpp: the Swift parameter ABIs where clang
// accepts them (clang `SemaSwift::AddParameterABIAttr`,
// `checkExtParameterInfos`): pointers and references of any pointee,
// `swift_error_result` on a pointer or reference to an unqualified pointer
// of any pointee (`const void *`, `void **`, `int *`) right after
// `swift_context`, several `swift_indirect_result` parameters first, the
// same attribute twice, `swift_async_context` under any convention, the
// ABIs on a function pointer, a member function, a template instantiated
// with pointers, and matching redeclarations. spec/sema.md §4.5, WP-1745.
//@ expect: compile-ok
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
#define SC __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
#define CTX __attribute__((swift_context))
#define ACTX __attribute__((swift_async_context))
#define ERR __attribute__((swift_error_result))
#define IND __attribute__((swift_indirect_result))
struct S { int a; };
SC void v1(IND S *a, IND int &b, int x, CTX S &c, ERR const void **e);
SC void v1(IND S *a, IND int &b, int x, CTX S &c, ERR const void **e);
SC void v2(CTX void *c, ERR void ***e);
SC void v3(CTX void *c, ERR int **e);
SC void v4(CTX CTX void *c, CTX void *d);
SC void v5(CTX void *c, ERR void *&e);
SA void v6(IND void *o, CTX void *c);
void v7(ACTX void *a);
SA void v8(ACTX void *a, CTX void *c);
void (*fp)(CTX void *, ERR void **) SC = nullptr;
struct M {
  SC void m(CTX void *c, ERR void **e);
};
template <class T> SC void t(IND T o, CTX T c) {}
void use() {
  t<int *>(nullptr, nullptr);
  void (*q)(IND S *, IND int &, int, CTX S &, ERR const void **) SC = v1;
  (void)q;
}
