// [except.throw]/3, [expr.const]/10 (C++26, P3068R6): the note on an
// exception that leaves a constant evaluation names the exception
// object's type (the operand's static type without its sugar and
// top-level cv-qualifiers) by one rule however the operand was written:
// clang's spelling of that canonical type in a diagnostic (an inline
// namespace is left out only where it is redundant, at every depth; the
// unnamed namespace prints `(anonymous namespace)::`; a closure type
// `(lambda at F:L:C)`), except that a class or enumeration declared in a
// function prints through the function (`d1()::error`), as clang 22
// prints such a class as the scope of another name, so that no spelling
// can equal a namespace-scope class's; template arguments follow the same
// rule. Before WP-3408 the note printed the operand's type as written
// (`lib::v0::error` and `lib::error` for one class, a bare `error` for a
// class in the unnamed namespace and a local class; reduced from a
// tooling client's report). The same types in clang's own diagnostics:
// thrown-type-spelling-clang.cpp.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-fail
//@ strict-diags
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:28 note "unhandled exception of type 'lib::error' with content {1} thrown from here"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:28 note "unhandled exception of type 'lib::error' with content {1} thrown from here"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:28 note "unhandled exception of type 'lib::error' with content {1} thrown from here"
//@ diag: 74:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:28 note "unhandled exception of type 'app::lib::error' with content {2} thrown from here"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:52 note "unhandled exception of type 'app::lib::error' with content {2} thrown from here"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:52 note "unhandled exception of type 'm::v1::e' with content {2} thrown from here"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 79:28 note "unhandled exception of type '(anonymous namespace)::error' with content {3} thrown from here"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 73:52 note "unhandled exception of type '(anonymous namespace)::error' with content {2} thrown from here"
//@ diag: 91:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:53 note "unhandled exception of type 'd1()::error' with content {4} thrown from here"
//@ diag: 92:15 error "static assertion expression is not an integral constant expression"
//@ diag: 86:64 note "unhandled exception of type 'd2()::o::in' with content {4} thrown from here"
//@ diag: 93:15 error "static assertion expression is not an integral constant expression"
//@ diag: 87:46 note "unhandled exception of type 'd3()::E' with content 4 thrown from here"
//@ diag: 94:15 error "static assertion expression is not an integral constant expression"
//@ diag: 88:70 note "unhandled exception of type 'q::d4(int)::error' with content {4} thrown from here"
//@ diag: 95:15 error "static assertion expression is not an integral constant expression"
//@ diag: 89:66 note "unhandled exception of type 'd5()::L' with content {4} thrown from here"
//@ diag: 96:15 error "static assertion expression is not an integral constant expression"
//@ diag: 90:49 note "unhandled exception of type 'outer::box<d6()::L>' with content {4} thrown from here"
//@ diag: 99:15 error "static assertion expression is not an integral constant expression"
//@ diag: 98:44 note "unhandled exception of type '(lambda at "
//@ diag: 104:15 error "static assertion expression is not an integral constant expression"
//@ diag: 102:28 note "unhandled exception of type 'outer::box<lib::error>' with content {5} thrown from here"
//@ diag: 105:15 error "static assertion expression is not an integral constant expression"
//@ diag: 103:28 note "unhandled exception of type 'outer::box<lib::error>::nested' with content {5} thrown from here"
//@ diag: 108:15 error "static assertion expression is not an integral constant expression"
//@ diag: 107:67 note "unhandled exception of type 'const app::lib::error *' with content &e thrown from here"
namespace lib { inline namespace v0 { struct error { int c; }; } }
namespace app { namespace lib { inline namespace v0 { struct error { int c; }; } } }
namespace { struct error { int c; }; }
namespace outer { template <class T> struct box { int c; struct nested { int c; }; }; }
namespace m { inline namespace v1 { struct e { int c; }; } inline namespace v2 { struct e { int c; }; } }
using alias = lib::v0::error;

// Inline namespace at depth 1, written with and without it, and through
// an alias: one spelling.
constexpr int a1() { throw lib::v0::error{1}; }
constexpr int a2() { throw lib::error{1}; }
constexpr int a3() { throw alias{1}; }
static_assert(a1());
static_assert(a2());
static_assert(a3());
// Inline namespace at depth 2, written and through a template parameter.
constexpr int b1() { throw app::lib::v0::error{2}; }
template <class T> constexpr int thrower() { throw T{2}; }
static_assert(b1());
static_assert(thrower<app::lib::error>());
// An inline namespace that is not redundant stays (m::e is ambiguous).
static_assert(thrower<m::v1::e>());
// The unnamed namespace.
constexpr int c1() { throw error{3}; }
static_assert(c1());
static_assert(thrower<error>());
// A local class, a class nested in one, a local enumeration, a local
// class of a function in a namespace, of a specialization, and as a
// template argument: through the function.
constexpr int d1() { struct error { int c; }; throw error{4}; }
constexpr int d2() { struct o { struct in { int c; }; }; throw o::in{4}; }
constexpr int d3() { enum E { x = 4 }; throw x; }
namespace q { constexpr int d4(int) { struct error { int c; }; throw error{4}; } }
template <class T> constexpr int d5() { struct L { T c; }; throw L{4}; }
constexpr int d6() { struct L { int c; }; throw outer::box<L>{4}; }
static_assert(d1());
static_assert(d2());
static_assert(d3());
static_assert(q::d4(0));
static_assert(d5<int>());
static_assert(d6());
// A closure type: by its location.
constexpr int e1() { auto l = [] {}; throw l; }
static_assert(e1());
// Class template specializations and a nested class: canonical
// arguments by the same rule.
constexpr int f1() { throw outer::box<lib::v0::error>{5}; }
constexpr int f2() { throw outer::box<alias>::nested{5}; }
static_assert(f1());
static_assert(f2());
// cv and pointer spellings unchanged.
constexpr int g1() { static constexpr app::lib::error e{6}; throw &e; }
static_assert(g1());
