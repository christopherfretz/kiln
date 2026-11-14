// [expr.const.const]/2 (P4101R1), [dcl.constexpr]/6: a block-scope
// `constexpr` local of a class with `std::meta::info` members that only
// consteval calls read. Holding only null reflections, it is not an
// immediate object: an ordinary automatic object with run-time storage
// at -O0, as any other constexpr local (spec/codegen.md "Constexpr
// locals"). Holding a non-null reflection, it is an immediate object and
// has none. Either way every read is an immediate invocation folded to a
// constant, so the program's behaviour is the same; reduced from real
// code, in a plain function, a function template instance (the reads in a
// designated initializer) and a function template whose template
// parameter is a reflection. Compared with GCC 16.1, which emits no
// storage for either (P2996R13 consteval-only types). WP-1410.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 1 1 3 1 1 3 1 1
#include <meta>
#include <cstdio>

struct tag { std::meta::info on; std::meta::info by; };
struct index_t { bool a; bool b; tag t; int n; };
struct out { int x; int y; bool z; };

consteval index_t make(bool reflect) {
    index_t r{};
    r.b = true;
    r.n = 3;
    if (reflect) r.t.on = ^^int;
    return r;
}
consteval bool pick(const index_t& i) { return i.b; }
consteval int count(const index_t& i) { return i.n; }

int null_holder() {
    constexpr index_t idx = make(false);
    const bool b = pick(idx);
    return b ? 1 : 0;
}

int immediate() {
    constexpr index_t idx = make(true);
    const bool b = pick(idx);
    return b ? 1 : 0;
}

template <bool Reflect> out designated() {
    constexpr index_t idx = make(Reflect);
    return out{.x = count(idx), .y = pick(idx) ? 1 : 0, .z = pick(idx)};
}

template <std::meta::info R> int by_reflection() {
    constexpr index_t idx = make(R != std::meta::info{});
    return pick(idx) ? 1 : 0;
}

int main() {
    out a = designated<false>();
    out b = designated<true>();
    std::printf("%d %d %d %d %d %d %d %d\n", null_holder(), immediate(), a.x, a.y, a.z ? 1 : 0, b.x,
                b.y, by_reflection<^^int>());
}
