// [meta.reflection.substitute] and its Example 1 (fn2),
// [dcl.spec.auto.general] and its note, [temp.inst],
// [temp.deduct.general] (P2996R13): can_substitute on a member function
// template with a default template argument and a decltype(auto) return
// type, with an empty argument list, names the specialization box::put<void>;
// whether its type still contains an undeduced placeholder decides the
// answer, so its definition is instantiated to deduce it, and the
// static_assert that fails there is outside the immediate context: the
// program is ill-formed, as it is for an ordinary call (the third case)
// and for substitute with the argument spelled out. Reduced from real
// code. GCC 16.1 answers true for a member template without instantiating
// it (docs/status/deviations.md, WP-1674, WP-1929).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not instantiate a member function template to deduce its return type in can_substitute (WP-1929)
//@ strict-diags
//@ diag: 34:19 error "static assertion failed due to requirement '!std::is_void_v<void>': E must not be void"
//@ diag: 38:32 note "in instantiation of function template specialization 'box::put<void>' requested here"
//@ diag: 38:32 note "while substituting template arguments into 'box::put' for std::meta::substitute"
//@ diag: 39:15 note "in call to 'asks()'"
//@ diag: 44:19 error "static assertion failed due to requirement '!std::is_void_v<void>': E must not be void"
//@ diag: 48:32 note "in instantiation of function template specialization 'box2::put<void>' requested here"
//@ diag: 48:32 note "while substituting template arguments into 'box2::put' for std::meta::substitute"
//@ diag: 53:19 error "static assertion failed due to requirement '!std::is_void_v<void>': E must not be void"
//@ diag: 57:30 note "in instantiation of function template specialization 'box3::put<void>' requested here"
#include <meta>
#include <type_traits>
#include <vector>

struct box {
  template <typename E = void>
  decltype(auto) put() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return 0;
  }
};
consteval bool asks() { return std::meta::can_substitute(^^box::put, std::vector<std::meta::info>{}); }
static_assert(asks() || !asks());

struct box2 {
  template <typename E = void>
  decltype(auto) put() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return 0;
  }
};
constexpr std::meta::info r2 = std::meta::substitute(^^box2::put, {^^void});

struct box3 {
  template <typename E = void>
  decltype(auto) put() {
    static_assert(!std::is_void_v<E>, "E must not be void");
    return 0;
  }
};
int call(box3& b) { return b.put(); }

int main() {}
