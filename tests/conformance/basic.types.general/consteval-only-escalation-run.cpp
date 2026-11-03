//@ expect: run
//@ args: -freflection
//@ oracle: none
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 2 1 2 1
// [expr.const] P2996R13: a consteval-only expression in an immediate-
// escalating function (a lambda's call operator, a constexpr function
// template's specialization) makes it immediate; its calls are then
// constant and the program runs with no reflection left to emit.
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
template <class T> struct Box {
  info r = ^^T;
  constexpr bool is_int() const { return r == ^^int; }
};
template <class T> constexpr int pick() { info l = ^^T; return l == ^^int ? 1 : 2; }
template <class T> int branch() { if constexpr (^^T == ^^int) return 1; else return 2; }
int main() {
  auto l = [] { return 1 + (^^int == ^^int); };
  constexpr Box<int> b;
  static_assert(b.is_int());
  printf("%d %d %d %d\n", l(), pick<int>(), pick<long>(), branch<int>());
}
