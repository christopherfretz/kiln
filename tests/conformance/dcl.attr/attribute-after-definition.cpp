// [clang ext] An attribute that a redeclaration adds after the entity's
// definition is ignored with "attribute declaration must precede
// definition" and a note at the definition (clang
// `Sema::checkNewAttributesAfterDef`, -Wignored-attributes): for functions
// (a deleted one and a function template too), variables, classes, class
// templates and enumerations. Not when the definition or a declaration
// before it already has one of that kind; not on an explicit
// specialization of a class template; not for C11 `_Noreturn` or the
// function type attributes (GNU `noreturn`, calling conventions); and,
// from clang 22, not for `[[deprecated]]`, `[[nodiscard]]` and
// `[[maybe_unused]]` in the standard syntax with any scope
// ([dcl.attr.deprecated]/3, [dcl.attr.nodiscard], [dcl.attr.unused]/4 let
// a redeclaration add them); their GNU spellings still warn. spec/sema.md
// §4.5, WP-1807.
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
//@ diag: 51:26 warning "attribute declaration must precede definition"
//@ diag: 50:6 note "previous definition is here"
//@ diag: 53:16 warning "attribute declaration must precede definition"
//@ diag: 52:6 note "previous definition is here"
//@ diag: 55:16 warning "attribute declaration must precede definition"
//@ diag: 54:5 note "previous definition is here"
//@ diag: 57:30 warning "attribute declaration must precede definition"
//@ diag: 56:12 note "previous definition is here"
//@ diag: 59:23 warning "attribute declaration must precede definition"
//@ diag: 58:8 note "previous definition is here"
//@ diag: 61:21 warning "attribute declaration must precede definition"
//@ diag: 60:6 note "previous definition is here"
//@ diag: 63:27 warning "attribute declaration must precede definition"
//@ diag: 62:12 note "previous definition is here"
//@ diag: 65:28 warning "attribute declaration must precede definition"
//@ diag: 64:20 note "previous definition is here"
//@ diag: 67:16 warning "attribute declaration must precede definition"
//@ diag: 66:15 note "previous definition is here"
//@ diag: 69:26 warning "attribute declaration must precede definition"
//@ diag: 68:6 note "previous definition is here"
//@ diag: 71:46 warning "attribute declaration must precede definition"
//@ diag: 70:25 note "previous definition is here"
//@ diag: 73:42 warning "attribute declaration must precede definition"
//@ diag: 72:27 note "previous definition is here"
//@ diag: 75:46 warning "attribute declaration must precede definition"
//@ diag: 74:18 note "previous definition is here"
//@ diag: 77:16 warning "attribute declaration must precede definition"
//@ diag: 76:5 note "previous definition is here"
//@ diag: 79:16 warning "attribute declaration must precede definition"
//@ diag: 78:5 note "previous definition is here"
//@ diag: 81:16 warning "attribute declaration must precede definition"
//@ diag: 80:5 note "previous definition is here"
void f1() {}
void f1() __attribute__((cold));
void f2() {}
__attribute__((noinline)) void f2();
int v1 = 0;
__attribute__((used)) extern int v1;
inline int v2 = 1;
extern int v2 __attribute__((used));
struct S1 {};
struct __attribute__((packed)) S1;
enum E1 { e1 };
enum __attribute__((flag_enum)) E1;
enum class E2 { x };
enum class __attribute__((flag_enum)) E2;
namespace N { void g() {} }
void N::g() __attribute__((cold));
constexpr int ce() { return 1; }
__attribute__((cold)) constexpr int ce();
void dl() = delete;
void dl() __attribute__((cold));
template <class T> void t1(T) {}
template <class T> void t1(T) __attribute__((cold));
template <class T> struct TS {};
template <class T> struct __attribute__((packed)) TS;
template <> void t1<int>(int) {}
template <> void t1<int>(int) __attribute__((cold));
int f3() { return 0; }
__attribute__((deprecated)) int f3();
int f4() { return 0; }
__attribute__((warn_unused_result)) int f4();
int f5() { return 0; }
__attribute__((unused)) int f5();

// Accepted.
void k1() __attribute__((cold));
void k1() {}
void k1() __attribute__((cold));
int k2() { return 0; }
[[nodiscard]] int k2();
int k3() { return 0; }
[[deprecated("use k4")]] int k3();
int k4() { return 0; }
[[maybe_unused]] int k4();
int k5() { return 0; }
[[gnu::deprecated]] int k5();
int k6() { return 0; }
[[gnu::unused]] int k6();
template <class T> struct TS2 {};
template <> struct TS2<int> {};
template <> struct __attribute__((packed)) TS2<int>;
void k7() {}
_Noreturn void k7();
int k8 = 0;
extern int k8;
void k9() {}
__attribute__((enable_if(1, ""))) void k9();
