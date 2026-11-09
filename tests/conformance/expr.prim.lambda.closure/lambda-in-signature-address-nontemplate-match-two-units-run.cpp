// [expr.prim.lambda.closure]/2, [over.over]/3, [temp.deduct.funcaddr]:
// for the address of an overload set, clang deduces every function
// template's specialization from the target type even when a
// non-template matches it ([over.over]/5 then prefers the non-template),
// and forming `fd<long>` numbers its return type's lambda in the pattern
// (spec/abi.md §8.15, spec/sema-lambda.md §3.5): the later `fd<double>`'s
// closure is `UlvE0_`. Inputs/lambda-in-signature-address-nontemplate-
// match-defs.cpp calls clang's name, so the program links only if kiln
// deduces the templates too (WP-1143).
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-address-nontemplate-match-defs.cpp
//@ stdout: 2 1
#include <cstdio>
template <class T> auto fd(T) -> decltype([] { return 1; }) { return {}; }
int fd(long) { return 1; }
int other_nontemplate();
int main() {
  int (*s)(long) = fd;
  std::printf("%d %d\n", s(0) + fd(2.0)(), other_nontemplate());
}
