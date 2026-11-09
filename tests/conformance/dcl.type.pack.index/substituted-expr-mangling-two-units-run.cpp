// [expr.prim.pack.index] (C++26, P2662) in a dependent signature of a
// member of a class template specialization ([temp.over.link]/5;
// spec/abi.md "Mangling", WP-1029): in `Y<1, 2>::g` the return type
// `decltype(u + I...[1])` keeps the pack-index expression after the
// class's arguments are substituted, so clang 19 and 22 write it as
// nothing (`DTplfp_E`). kiln's substitution folds it to the selected
// element and writes `DTplfp_Li2EE`, so the reference does not link.
// Inputs/substituted-expr-mangling-defs.cpp defines the specialization
// under clang's name with an asm label.
//@ xfail(kiln): WP-1029 follow-up, template substitution folds a pack-index expression in a dependent decltype to its selected element (kiln_sema templ/pack.rs pack_indexing_expr)
//@ expect: run
//@ args: %{test_dir}/Inputs/substituted-expr-mangling-defs.cpp
//@ stdout: 3
extern "C" int printf(const char *, ...);
template <int... I> struct Y {
  template <class U> static auto g(U u) -> decltype(u + I...[1]);
};
int main() { printf("%d\n", Y<1, 2>::g(1)); }
