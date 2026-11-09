// C++14 [except.spec]/2 (removed in C++17, [diff.cpp14.except]): an
// exception-specification may appear only on a function declarator, or on
// the function type a top-level pointer, reference or pointer to member of a
// declaration denotes. Clang `CheckDistantExceptionSpec` rejects a pointer,
// reference, array or pointer to member formed from a pointer (or pointer to
// member) to a function type with a specification: at the declarator-id, at
// the place an abstract declarator's name would go, and for a pointer to
// member at its nested-name-specifier. The type is still built, but the
// declaration is invalid, so the rest of its declaration group is not
// declared (g2). kiln accepted all of these. WP-1102.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 41:9 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 42:11 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 43:9 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 44:8 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 45:16 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 46:8 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 47:12 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 49:15 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 50:16 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 50:40 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 51:15 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 52:23 error "exception specifications are not allowed in type aliases"
//@ diag: 52:19 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 54:11 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 55:11 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 57:35 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 58:11 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 59:24 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 61:11 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 62:24 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 63:24 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 65:9 error "exception specifications are not allowed beyond a single level of indirection"
//@ diag: 66:14 error "use of undeclared identifier 'g2'"

struct S;
template <class T> struct X {};
void (**j)() throw(int);
void (**(*h())())() throw(int);
void (**k)() noexcept(false);
void (*arr[2])() throw();
extern void (*&r)() noexcept;
void (*S::*mp)() noexcept;
void (S::**mpp)() noexcept;
void f() throw();
decltype(&f) *via_decltype;
void p(void (**pp)() noexcept, void (**)() noexcept);
void q(void (*qa[])() throw());
using A = void (**)() noexcept;
struct M {
  void (**m)() throw();
  void (**mf())() throw();
};
template <class T> void t(void (**)() noexcept);
X<void (**)() throw()> xa;
void *n = new (void (**)() throw());
void g() {
  void (**loc)() noexcept = 0;
  (void)sizeof(void (**)() noexcept);
  (void)(void (*const *)() throw())0;
}
void (**g1)() throw(), (*g2)();
void use() { g2(); }
