// `sizeof`, `alignof` and GNU `__alignof__` operands in function
// template signatures ([expr.sizeof], [expr.alignof], [temp.over.link]/5;
// spec/abi.md "Mangling", WP-1226). The specializations below are
// declared here and defined in Inputs/alignof-operand-defs.cpp under the
// names clang 19 and 22 give them (asm labels), so this links only if
// kiln's names are clang's: `u11__alignof__ <type> E` for a dependent
// `__alignof__` (in a default template argument reached through
// `storage<sizeof(T)>::type`, libstdc++'s `aligned_storage` shape),
// `u11__alignof__ <template-arg> E` for an expression operand, `at`/`st`
// for `alignof`/`sizeof` of a type. `std::aligned_storage` itself (whose
// signature kiln rejected with "cannot yet mangle expression type
// UnaryExprOrTypeTraitExpr") is the in-unit `q`.
//@ expect: run
//@ args: -Wno-deprecated-declarations %{test_dir}/Inputs/alignof-operand-defs.cpp
//@ stdout: 0 2 3 4 5
#include <type_traits>
extern "C" int printf(const char *, ...);
template <unsigned long L> struct msa {
  struct type {
    alignas(16) unsigned char data[L];
  };
};
template <unsigned long L, unsigned long Al = __alignof__(typename msa<L>::type)>
struct storage {
  struct type {
    alignas(Al) unsigned char data[L];
  };
};
template <class T, unsigned long N> struct B {};
template <class T> int f(typename storage<sizeof(T)>::type *p);
template <class T> int g(B<T, alignof(T)> *);
template <class T> int h(B<T, __alignof__(T) + 1> *);
template <class T> int k(T t, B<T, __alignof__(t) + sizeof(T)> *);
template <class T> int q(typename std::aligned_storage<sizeof(T)>::type *p) {
  return sizeof(*p) == sizeof(typename std::aligned_storage<sizeof(T)>::type) ? 5 : 0;
}
int main() {
  storage<sizeof(long)>::type s;
  std::aligned_storage<sizeof(long)>::type t;
  printf("%d %d %d %d %d\n", f<long>(&s), g<int>(nullptr), h<int>(nullptr),
         k<int>(1, nullptr), q<long>(&t));
}
