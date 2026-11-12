// [stmt.expand], [expr.const]/25.3 (P2996R13): the well-formed spelling
// of a run-time use of a consteval-only expansion variable's member: read
// it in a constexpr variable's initializer (manifestly constant-evaluated),
// then use that variable at run time. In every form of
// consteval-only-element-neg.cpp (enumerating, iterating, destructuring,
// `constexpr const auto&`, `constexpr auto&&`, a structured binding, a
// function template), and the uses that stay allowed without it: a
// `static_assert`, an unevaluated operand (`sizeof`, `decltype`), an
// `if consteval` block, and a lambda whose call operator is
// immediate-escalating ([expr.const]/20) and so becomes an immediate
// function, called in a constant expression (with `constexpr E e`: GCC
// 16.1 rejects a lambda naming a `constexpr auto` expansion variable, "use
// of 'e' before deduction of 'auto'"; that spelling is kiln-only, in
// consteval-only-element-lambda.cpp). WP-1330. GCC 16.1 compiles and runs
// the same file.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout-file: consteval-only-element-run.stdout
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
struct P { E a; E b; };
constexpr P two{{^^int, "a"}, {^^long, "b"}};
void h() { constexpr E e = plan[0]; constexpr const char* n = e.name; std::puts(n); }
void g_enum() {
  template for (constexpr E e : {plan[0], plan[1]}) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void g_iter() {
  template for (constexpr auto e : plan) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void g_dest() {
  template for (constexpr auto e : two) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void g_ref() {
  template for (constexpr const auto& e : plan) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void g_fwd() {
  template for (constexpr auto&& e : plan) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void g_bind() {
  template for (constexpr auto [r, n] : plan) {
    constexpr const char* s = n;
    std::puts(s);
  }
}
template <class T>
void t() {
  template for (constexpr auto e : plan) {
    constexpr const char* n = e.name;
    std::puts(n);
  }
}
void allowed() {
  template for (constexpr auto e : plan) {
    static_assert(e.name[0] != 0);
    std::printf("%d ", (int)sizeof(e.name) == (int)sizeof(const char*));
    decltype(e.name) x = nullptr;
    if consteval {
      std::puts(e.name);
    }
    std::printf("%d ", x == nullptr);
  }
  template for (constexpr E e : plan) {
    constexpr int len = [] { return e.name[0] == 'i' ? 3 : 4; }();
    std::printf("%d\n", len);
  }
}
int main() {
  h(), g_enum(), g_iter(), g_dest(), g_ref(), g_fwd(), g_bind();
  t<int>();
  allowed();
}
