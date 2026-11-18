// [expr.typeid]/2, [except.handle]/3, [basic.link]/4: a closure type
// created by the initializer of a `static`, `const` or unnamed-namespace
// variable, an unnamed class at namespace scope and a class in an unnamed
// namespace (here only declared, `typeid(I *)`) are different types in
// each translation unit. kiln names the first two clang's way (`$_N`,
// numbered per unit, so both units write the same name) and gives every
// one an internal `type_info`; under libstdc++, which compares names
// unless one starts with `*`, the twins compared equal and a handler for
// one caught the other's exception. kiln now writes GCC's `*` for these
// forms too (spec/abi.md "RTTI names of unit-local types", WP-1679).
// Within a unit each type equals itself. Found running GCC's C++
// testsuite.
//@ expect: run
//@ std: c++20
//@ unit: Inputs/unit-local-marker-forms-a.cpp
//@ unit: Inputs/unit-local-marker-forms-b.cpp
//@ skip(clang): clang 22 writes no unit-local marker, so libstdc++ compares the twins equal
//@ stdout: static-closure: eq=0 ne=1 before-one=1 name-same=1 self=1
//@ stdout: const-closure: eq=0 ne=1 before-one=1 name-same=1 self=1
//@ stdout: anon-closure: eq=0 ne=1 before-one=1 name-same=1 self=1
//@ stdout: unnamed: eq=0 ne=1 before-one=1 name-same=1 self=1
//@ stdout: incomplete: eq=0 ne=1 before-one=1 name-same=1 self=1
//@ stdout: catch closure own: 1 1 other: 0 0
//@ stdout: catch unnamed own: 1 1 other: 0 0
#include "Inputs/unit-local-marker-forms.h"
#include <cstdio>
#include <cstring>

static void row(const char *what, const std::type_info *x, const std::type_info *y) {
  // Exactly one of two unequal `type_info`s is before the other.
  std::printf("%s: eq=%d ne=%d before-one=%d name-same=%d self=%d\n", what, *x == *y, *x != *y,
              x->before(*y) != y->before(*x), std::strcmp(x->name(), y->name()) == 0,
              *x == *x && !x->before(*x));
}

int main() {
  const Forms &a = forms_a, &b = forms_b;
  row("static-closure", a.static_closure, b.static_closure);
  row("const-closure", a.const_closure, b.const_closure);
  row("anon-closure", a.anon_closure, b.anon_closure);
  row("unnamed", a.unnamed, b.unnamed);
  row("incomplete", a.incomplete, b.incomplete);
  std::printf("catch closure own: %d %d other: %d %d\n", a.catch_closure(a.throw_closure),
              b.catch_closure(b.throw_closure), a.catch_closure(b.throw_closure),
              b.catch_closure(a.throw_closure));
  std::printf("catch unnamed own: %d %d other: %d %d\n", a.catch_unnamed(a.throw_unnamed),
              b.catch_unnamed(b.throw_unnamed), a.catch_unnamed(b.throw_unnamed),
              b.catch_unnamed(a.throw_unnamed));
}
