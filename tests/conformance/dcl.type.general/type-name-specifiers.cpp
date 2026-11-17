//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 27:16 error "type name does not allow function specifier to be specified"
//@ diag: 28:16 error "type name does not allow function specifier to be specified"
//@ diag: 29:11 error "type name does not allow function specifier to be specified"
//@ diag: 30:14 error "type name does not allow function specifier to be specified"
//@ diag: 31:10 error "type name does not allow function specifier to be specified"
//@ diag: 32:3 error "type name does not allow function specifier to be specified"
//@ diag: 33:16 error "type name does not allow constexpr specifier to be specified"
//@ diag: 34:3 error "type name does not allow consteval specifier to be specified"
//@ diag: 35:3 error "type name does not allow constinit specifier to be specified"
//@ diag: 36:23 error "type name does not allow storage class to be specified"
//@ diag: 36:16 error "type name does not allow function specifier to be specified"
//@ diag: 36:30 error "type name does not allow constexpr specifier to be specified"
//@ diag: 37:18 error "type name does not allow function specifier to be specified"
//@ diag: 39:17 error "type name does not allow function specifier to be specified"
//@ diag: 40:17 error "type name does not allow constexpr specifier to be specified"
// [dcl.type.general]/1: a type-specifier-seq (every type-id, a
// new-type-id, an exception-declaration) holds type-specifiers only, so
// no function specifier and no constexpr kind (clang 22
// `ParseSpecifierQualifierList`; each specifier is dropped). The
// storage class first, then the function specifiers, then constexpr.
// Found running GCC's C++ testsuite.
template <class T> struct X {};
int a = sizeof(virtual int);
int b = sizeof(inline int);
using T = explicit int;
int *p = new virtual int;
int c = (explicit int)1;
X<inline int> xi;
int d = sizeof(constexpr int);
X<consteval int> xe;
X<constinit int> xn;
int q = sizeof(inline static constexpr int);
auto l = []() -> inline int { return 0; };
void f() {
  try {} catch (virtual int x) {}
  try {} catch (constexpr int x) {}
  try {} catch (const int &x) {}
}
int ok = sizeof(const volatile int) + alignof(unsigned long);
