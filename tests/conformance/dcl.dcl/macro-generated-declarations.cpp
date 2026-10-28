// [dcl.dcl], [cpp.include]: declarations whose tokens all come from macro
// expansion and included files are ordinary declarations.
//@ expect: compile-ok
//@ strict-diags
#define MEMBER(type, name) type name;
struct Record {
#include "Inputs/members.def"
};
#undef MEMBER
extern Record probe;
#define MEMBER(type, name) static_assert(sizeof(probe.name) == sizeof(type));
#include "Inputs/members.def"
#undef MEMBER
#define CONST(name, ...) constexpr Record name{__VA_ARGS__};
CONST(r, 1, 2, 3)
static_assert(r.id == 1 && r.flags == 2 && r.stamp == 3);
#define FN(name, body) constexpr int name(int v) { return body; }
FN(twice, v * 2)
FN(next, v + 1)
static_assert(twice(next(4)) == 10);
