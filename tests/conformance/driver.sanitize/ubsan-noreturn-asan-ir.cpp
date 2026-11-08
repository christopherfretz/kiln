// UBSan unreachable with ASan: the call that does not return loses
// `noreturn`, so the frontend puts `__asan_handle_no_return` before it
// itself (clang `EmitCall`), also before an `invoke`. The frontend output
// matches clang's (checked with value names stripped on both sides), but
// clang keeps value names under -fsanitize=address and clang 22 annotates
// the allocas ASan instruments; kiln does neither.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=address,unreachable -Xclang -disable-llvm-passes
//@ xfail(kiln): WP-871 follow-up, no value names under -fsanitize=address and no !annotation alloca_name_altered (clang 22)
[[noreturn]] void die();
struct S {
  [[noreturn]] static void st();
  [[noreturn]] void stop();
};
S s;
void a() { die(); }
void b() { S::st(); }
void c() { s.stop(); }
void d() noexcept { die(); }
struct G { G(); ~G(); };
G g; // its initializer calls nothing that does not return
void e() try { die(); } catch (...) {}
