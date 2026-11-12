// [stmt.expand], [expr.const]/25.3 (P2996R13), [basic.types.general]/12:
// the element variable of an expansion statement whose class has a
// std::meta::info member is a variable of consteval-only type, so naming
// it in a run-time expression (here `e.name`, an argument of a run-time
// call) is a potentially-evaluated expression of consteval-only type
// outside an immediate function context: ill-formed in a function that is
// not immediate-escalating, as for a plain `constexpr E e = plan[0];`
// (line 40). Equally ill-formed under P4101R1 ([expr.const.imm]/2.1 with
// [expr.const.const]/5.1: a glvalue that refers to an immediate object).
// Every form: enumerating, iterating, destructuring (over an aggregate of
// two E), `constexpr auto&&`, a structured binding as the
// for-range-declaration, and in a function template, checked once at
// its instantiation (line 48). Before WP-1330 kiln accepted every
// expansion form: the pattern's `constexpr auto e` has a dependent type,
// so its use was never recorded as a consteval-only leaf. GCC 16.1
// rejects each (it reports each once per expansion, kiln once per
// location). The read into a non-constexpr local, which GCC accepts, is
// in consteval-only-element-copy-neg.cpp; the well-formed spelling in
// consteval-only-element-run.cpp.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 40:47 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 41:77 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 42:66 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 43:65 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 44:67 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 45:71 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 46:72 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 48:61 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 51:3 note "in instantiation of function template specialization 't<int>' requested here"
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
struct P { E a; E b; };
constexpr P two{{^^int, "a"}, {^^long, "b"}};
void h() { constexpr E e = plan[0]; std::puts(e.name); }
void g_enum() { template for (constexpr E e : {plan[0], plan[1]}) std::puts(e.name); }
void g_iter() { template for (constexpr auto e : plan) std::puts(e.name); }
void g_dest() { template for (constexpr auto e : two) std::puts(e.name); }
void g_fwd() { template for (constexpr auto&& e : plan) std::puts(e.name); }
void g_bind() { template for (constexpr auto [r, n] : plan) std::puts(n); }
void g_ref() { template for (constexpr const auto& e : plan) std::puts(e.name); }
template <class T>
void t() { template for (constexpr auto e : plan) std::puts(e.name); }
int main() {
  h(), g_enum(), g_iter(), g_dest(), g_fwd(), g_bind(), g_ref();
  t<int>();
}
