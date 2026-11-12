// [expr.const.const], [expr.const.imm] (P4101R1, the value-based model;
// formerly P2996R13's consteval-only types): a variable whose value holds
// a non-null reflection is constexpr, an expression whose value does
// appears only in an immediate function context (an immediate-escalating
// function escalates instead), and one holding only the null reflection
// is unrestricted, as is a function whose type holds std::meta::info
// (only its body's values are judged; WP-1361).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 25:12 error "'cg' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 29:3 error "'w' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 37:30 error "consteval-only value outside an immediate function context"
//@ diag: 44:3 error "consteval-only value outside an immediate function context"
//@ diag: 46:7 error "consteval-only value outside an immediate function context"
//@ diag: 48:13 error "consteval-only value outside an immediate function context"
//@ diag: 49:14 error "consteval-only value outside an immediate function context"
//@ diag: 51:31 error "consteval-only value outside an immediate function context"
//@ diag: 57:21 error "consteval-only value outside an immediate function context"
//@ diag: 52:37 error "consteval-only value outside an immediate function context"
//@ diag: 54:10 note "in instantiation of function template specialization 'g<int>' requested here"
using info = decltype(^^int);
constexpr info r = ^^int;
const info cg = ^^int;                 // a non-null reflection, not constexpr
info gv;                               // OK: the null reflection
extern info ex;                        // OK: no value
struct W { info m; };
W w{^^int};                            // a class with an info member
constexpr W cw{^^int};                 // OK
info* gp;                              // OK: a null pointer
struct S { static constexpr info sm = ^^int; };  // OK
constexpr bool b1 = r == ^^int;        // OK: manifestly constant-evaluated
bool b2 = r == ^^int;                  // OK: constant initialization
static_assert(r == ^^int);
info f1();                             // OK: not consteval
constexpr info f2() { return ^^int; }  // a plain constexpr function does not escalate
consteval info f3() { return ^^int; }  // OK
void f4(info);                         // OK: a parameter of type info
auto lam = [](info x) { return x == ^^int; };  // OK: the call operator escalates
template <info R> int tf() { return 1; }
int u = tf<^^int>();                   // OK: a template argument
void plain() {
  ^^int;                               // a plain function
  constexpr info lr = ^^int;           // OK
  if (lr == ^^int) {}                  // a run-time condition
  if constexpr (lr == ^^int) {}        // OK
  auto p = &r;                         // a pointer to info
  info loc = ^^int;
}
constexpr int cf() { info l = ^^int; return 0; }  // not a template: no escalation
template <class T> int g() { return ^^T == ^^int; }   // checked per instantiation
template <class T> int h() { if constexpr (^^T == ^^int) return 1; return 0; }  // OK
int gi = g<int>();
template <class T> constexpr bool tfn() { info l = ^^T; return l == ^^int; }
static_assert(tfn<int>());             // OK: escalates
int main() { return r == ^^int; }      // a run-time expression
