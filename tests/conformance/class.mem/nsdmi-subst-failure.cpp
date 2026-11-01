// [class.mem.general]/8, [class.base.init]/9.1, [temp.inst]/3: a default member
// initializer whose substitution fails is diagnosed where it is written (in the
// header here); a constructor that uses it then reports nothing more (clang
// `InstantiateInClassInitializer` marks the field invalid), whether the constructor
// is implicit or user-provided. Only the anchor below is expected in this file.
// (kiln instantiates the initializers with the class, clang when a constructor
// needs them, so the notes of the header's errors differ; spec/sema-templates.md.)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion failed: anchor"
#include "Inputs/nsdmi-subst-failure.h"

A<X> a;
C<X> c;
C<X> c2(1);
int uses() { return a.w + c.v + c2.v; }
A<X> make() { return {}; }
C<X> copy = c;

static_assert(false, "anchor");
