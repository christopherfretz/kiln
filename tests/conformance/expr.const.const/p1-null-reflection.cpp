// [expr.const.const]/1-3 (P4101R1): the null reflection is not a
// consteval-only value, so an object holding only null reflections is
// not an immediate object and needs no `constexpr`: a namespace-scope or
// local `std::meta::info`, a class with an `info` member, a pointer to
// `info`, a non-constexpr `const` one constant-initialized with the null
// reflection, and run-time reads and writes of such objects with null
// values. A non-constexpr reference may bind an immediate object (a
// reference is not an object). kiln only: GCC 16.1 implements the
// P2996R13 type rule and rejects every variable here (see
// docs/status/deviations.md); clang 19/22 lack reflection. Code
// generation for a run-time `info` is WP-1328 land 2, so this is
// syntax-only (kiln_codegen/tests/runtime_reflection.rs pins the
// "not supported yet" error).
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
using info = decltype(^^int);
struct S { info r; const char* name; };
struct D { info r{}; int n = 1; };

info gv;                                 // OK: the null reflection
S gs;                                    // OK
D gd;                                    // OK: a null default member initializer
info* gp;                                // OK: a null pointer
const info cn = info{};                  // OK: constant, but not consteval-only
extern info ex;                          // OK
constexpr info r = ^^int;
const info& ref = r;                     // OK: a reference is not an object
constexpr S cs{{}, "x"};                 // OK

void run() {
  info x;                                // OK
  x = info{};                            // OK: a null prvalue
  bool b = x == info{};                  // OK
  (void)b;
  S s{{}, "local"};                      // OK
  s.r = gv;                              // OK: run-time reads of null values
  static info st;                        // OK
  (void)st;
  info* p = &x;                          // OK
  (void)p;
  const char* n = cs.name;               // OK: `cs` is not an immediate object
  (void)n;
}
