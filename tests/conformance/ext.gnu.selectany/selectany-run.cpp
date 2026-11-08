// GNU `selectany` (WP-809; clang `SelectAnyAttr`): both units define the
// same variables from one header, so the program links only because each
// definition is `weak_odr` in a comdat; the linker keeps one object per
// variable, both units see the same address, and each dynamic
// initializer (in the variable's comdat) runs once.
//@ expect: run
//@ args: %{test_dir}/Inputs/selectany-other.cpp
//@ stdout: same 1 1 1 1 1 1 1
//@ stdout: values 42 42 7 6 1 5 9
//@ stdout: runs 1 1
#include <cstdio>
#include "Inputs/selectany.h"
int ctor_runs = 0;
static int calls = 0;
int next_value() { return ++calls; }
Counted::Counted() : v(++ctor_runs) {}
int main() {
  std::printf("same %d %d %d %d %d %d %d\n", other_answer() == &answer,
              other_limit() == &limit, other_table() == table,
              other_dynamic() == &dynamic, other_counted() == &counted,
              other_member() == &Holder::member, other_per_thread() == &per_thread);
  std::printf("values %d %d %d %d %d %d %d\n", answer, *ptr, limit,
              table[0] + table[1] + table[2], dynamic, Holder::member, per_thread);
  std::printf("runs %d %d\n", calls, ctor_runs);
}
