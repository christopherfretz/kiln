// [dcl.align]/1: an alignment-specifier may be applied to a variable or to
// a class data member, but not to a bit-field, a function parameter, an
// exception-declaration or a variable declared with the register
// storage-class-specifier; or to a class or enumeration (only a class:
// clang rejects an enumeration). On any other declaration it appertains
// to nothing.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-register
//@ diag: 23:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 24:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 25:10 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 26:8 error "'alignas' attribute cannot be applied to a function parameter"
//@ diag: 27:12 error "'alignas' attribute cannot be applied to a bit-field"
//@ diag: 27:43 error "'alignas' attribute cannot be applied to a bit-field"
//@ diag: 28:6 error "'alignas' attribute cannot be applied to an enumeration"
//@ diag: 29:12 error "'alignas' attribute cannot be applied to an enumeration"
//@ diag: 30:26 error "'alignas' attribute cannot be applied to a 'catch' variable"
//@ diag: 31:12 error "'alignas' attribute cannot be applied to a variable with 'register' storage class"
//@ diag: 32:38 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 33:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 34:15 error "'alignas' attribute only applies to variables, data members and tag types"
alignas(8) void fn();
alignas(8) typedef int T1;
using T3 alignas(8) = int;
void p(alignas(8) int x);
struct B { alignas(8) int bf : 3; int bf2 alignas(8) : 3; };
enum alignas(8) E { e };
enum class alignas(8) E2 { e2 };
void h() { try {} catch (alignas(8) int e) {} }
void q() { alignas(8) register int r2 = 0; (void)r2; }
struct M { alignas(8) static int sm; alignas(8) void mf(); };
alignas(8);
auto lam = [] alignas(8) () {};
// Accepted: variables, static and non-static data members, classes.
alignas(8) struct S0 { int i; } s0;
struct alignas(8) S1;
template <class T> alignas(16) T tv;
static_assert(alignof(S0) == 4 && alignof(M) == 1);
static_assert(__alignof(tv<char>) == 16 && __alignof(M::sm) == 8);
