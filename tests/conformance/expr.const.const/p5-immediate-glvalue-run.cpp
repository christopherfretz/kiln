// [expr.const.const]/5, [expr.const.imm]/1.2 (P4101R1): the portable
// spelling of a run-time read of a member of an immediate object reads
// it in a `constexpr` variable's initializer (manifestly
// constant-evaluated, an immediate function context), so only the
// resulting pointer reaches run time: in an expansion statement, from a
// constexpr local, from a namespace-scope variable and in a function
// template specialization. The immediate objects themselves get no
// storage (spec/codegen.md "Consteval-only variables"). Compared with GCC
// 16.1.
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ stdout: char short float bool int done
extern "C" int printf(const char*, ...);
using info = decltype(^^int);
struct Tag { info what; const char* label; };

constexpr Tag tags[] = {{^^char, "char"}, {^^short, "short"}};
constexpr Tag one{^^bool, "bool"};

template <class T> const char* name_of() {
  constexpr Tag t{^^T, "int"};
  constexpr const char* label = t.label;
  return label;
}

int main() {
  template for (constexpr auto t : tags) {
    constexpr const char* label = t.label;
    printf("%s ", label);
  }
  constexpr Tag t{^^float, "float"};
  constexpr const char* label = t.label;
  constexpr const char* global = one.label;
  constexpr bool is_bool = one.what == ^^bool;
  printf("%s %s %s done\n", label, is_bool ? global : "?", name_of<int>());
  return 0;
}
