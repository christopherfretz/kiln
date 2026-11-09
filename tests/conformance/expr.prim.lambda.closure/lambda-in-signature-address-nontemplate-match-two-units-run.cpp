// [expr.prim.lambda.closure]/2, [over.over]/3, [temp.deduct.funcaddr]:
// for the address of an overload set, clang deduces every function
// template's specialization from the target type even when a
// non-template matches it ([over.over]/5 then prefers the non-template),
// and forming `fd<long>` numbers its return type's lambda in the pattern
// (spec/abi.md §8.15): the later `fd<double>`'s closure is `UlvE0_`. Kiln
// skips the templates once a non-template matches, so it numbers
// `fd<double>` `UlvE_` and Inputs/lambda-in-signature-address-nontemplate-
// match-defs.cpp, which calls clang's name, does not link.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-address-nontemplate-match-defs.cpp
//@ stdout: 2 1
//@ xfail(kiln): WP-1086 follow-up, overload/address.rs does not deduce the function templates for an address once a non-template matches the target, so their specializations take no lambda numbers
#include <cstdio>
template <class T> auto fd(T) -> decltype([] { return 1; }) { return {}; }
int fd(long) { return 1; }
int other_nontemplate();
int main() {
  int (*s)(long) = fd;
  std::printf("%d %d\n", s(0) + fd(2.0)(), other_nontemplate());
}
