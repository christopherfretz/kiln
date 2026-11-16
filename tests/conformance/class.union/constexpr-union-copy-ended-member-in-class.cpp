// [class.copy.ctor]/14-15, [class.copy.assign]/12-13: the implicit copy of
// a class copies its members memberwise, and its union member by the
// union's object representation, so copying a class whose union member's
// active member an explicit destruction ended is a constant expression.
// GCC 16.1 accepts it; clang 22 rejects it ("subobject 'm' is not
// initialized": its trivial copy reads the whole class as one value)
// (spec/oven.md "Copying a union with an ended or no active member").
//@ expect: compile-ok
//@ xfail(clang): clang 22 reads the class as a value in its trivial copy and rejects the ended union member as uninitialized
#include <memory>
struct S { int x; };
union U { S m; int i; };
struct H { int k; U u; };
constexpr int copy_holder() { H h{1, {.m = {1}}}; std::destroy_at(&h.u.m); H g = h; return g.k; }
static_assert(copy_holder() == 1);
constexpr int assign_holder() { H h{1, {.m = {1}}}; std::destroy_at(&h.u.m); H g{2, {.i = 3}}; g = h; return g.k; }
static_assert(assign_holder() == 1);
