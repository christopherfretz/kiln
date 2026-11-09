// [expr.prim.pack.index] (C++26, P2662) in a dependent signature of a
// member of a class template specialization ([temp.over.link]/5;
// spec/abi.md §8.15, WP-1060): in `Y<1, 2>::g` the return type
// `decltype(u + I...[1])` keeps the pack-index expression after the
// class's arguments are substituted, so clang 19 and 22 write it as
// nothing (`DTplfp_E`), not as the selected element (`DTplfp_Li2EE`).
// Inputs/substituted-expr-mangling-defs.cpp defines the specialization
// under clang's name with an asm label, so the program links only if
// kiln's reference carries that name.
//@ expect: run
//@ args: %{test_dir}/Inputs/substituted-expr-mangling-defs.cpp
//@ stdout: 3
extern "C" int printf(const char *, ...);
template <int... I> struct Y {
  template <class U> static auto g(U u) -> decltype(u + I...[1]);
};
int main() { printf("%d\n", Y<1, 2>::g(1)); }
