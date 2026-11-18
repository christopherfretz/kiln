// [expr.typeid]/2, [except.handle]/3, [expr.dynamic.cast]/7,
// [basic.link]/4: a class in an unnamed namespace (and a local class of a
// function with internal linkage, a closure type there, and every type
// compounded from one) is a different type in each translation unit, so
// two units' twins of one name compare unequal, `before` orders them one
// way, a handler for one does not catch the other's exception, and a
// `dynamic_cast` to one fails on the other's object. libstdc++ compares
// `type_info` objects by name unless a name starts with `*`, which GCC
// writes in front of the name of a type local to the unit; kiln does the
// same under libstdc++ (spec/abi.md "RTTI names of unit-local types",
// WP-1651). `name()` is unchanged (libstdc++ skips the `*`). A type
// declared in a header (`Shared`) is the same in both units: equal, and
// caught across them. Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++20
//@ unit: Inputs/unit-local-type-info-a.cpp
//@ unit: Inputs/unit-local-type-info-b.cpp
//@ skip(clang): clang 22 writes no unit-local marker, so libstdc++ compares the twins equal
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
