// [expr.typeid]/2, [except.handle]/3, [expr.dynamic.cast]/7,
// [basic.link]/4: the libc++ twin of unit-local-type-info-libstdcxx-run.cpp
// (same units). libc++ on Linux compares `type_info` objects by the
// address of their names, and the `_ZTS` name of a type local to the unit
// is an `internal` symbol in each unit, so two units' same-named twins
// compare unequal and one unit's type compares equal to itself without
// GCC's `*` marker: kiln writes none under libc++, exactly as clang 22 and
// 19 on both Linux targets (spec/abi.md "RTTI names of unit-local types";
// the marker is libstdc++-only, WP-1651, re-checked by WP-1818).
//@ expect: run
//@ std: c++20
//@ args: -stdlib=libc++
//@ unit: Inputs/unit-local-type-info-a.cpp
//@ unit: Inputs/unit-local-type-info-b.cpp
//@ stdout: local: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: local-ptr: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: local-array: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: local-fn: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: fn-local: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: lambda: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: template: eq=0 ne=1 before-one=1 name-same=1
//@ stdout: shared: eq=1 ne=0 before-one=0 name-same=1
//@ stdout: self: eq=1 ne=0 before-one=0 name-same=1
//@ stdout: name: N12_GLOBAL__N_11SE
//@ stdout: catch own: 1 1
//@ stdout: catch other: 0 0
//@ stdout: catch other ptr: 0 0
//@ stdout: catch shared: 1 1
//@ stdout: cast own: 1 1 other: 0 0
#include "Inputs/unit-local-type-info.h"
#include <cstdio>
#include <cstring>

static void row(const char *what, const std::type_info &x, const std::type_info &y) {
  // Exactly one of two unequal `type_info`s is before the other.
  std::printf("%s: eq=%d ne=%d before-one=%d name-same=%d\n", what, x == y, x != y,
              x.before(y) != y.before(x), std::strcmp(x.name(), y.name()) == 0);
}

int main() {
  const Unit &a = unit_a, &b = unit_b;
  row("local", a.local, b.local);
  row("local-ptr", a.local_ptr, b.local_ptr);
  row("local-array", a.local_array, b.local_array);
  row("local-fn", a.local_fn, b.local_fn);
  row("fn-local", a.fn_local, b.fn_local);
  row("lambda", a.lambda, b.lambda);
  row("template", a.tmpl, b.tmpl);
  row("shared", a.shared, b.shared);
  row("self", a.local, a.local);
  std::printf("name: %s\n", a.local.name());
  std::printf("catch own: %d %d\n", a.catch_local(a.throw_local), b.catch_local(b.throw_local));
  std::printf("catch other: %d %d\n", a.catch_local(b.throw_local), b.catch_local(a.throw_local));
  std::printf("catch other ptr: %d %d\n", a.catch_local_ptr(b.throw_local_ptr),
              b.catch_local_ptr(a.throw_local_ptr));
  std::printf("catch shared: %d %d\n", a.catch_shared(b.throw_shared),
              b.catch_shared(a.throw_shared));
  Base *pa = a.make(), *pb = b.make();
  std::printf("cast own: %d %d other: %d %d\n", a.cast_local(pa), b.cast_local(pb),
              a.cast_local(pb), b.cast_local(pa));
  delete pa;
  delete pb;
}
