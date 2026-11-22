// The written-reference table (spec/ast.md §18.4) in declarative
// contexts (stage 3) after an error: a using-declarator, a nominated
// namespace, an alias target, a qualified declarator-id and a designator
// that name nothing are recorded with no entity and `unresolved`; the
// names before them keep theirs.
//@ expect: compile-fail
//@ written-refs: decl-errors.refs
//@ diag: 14:14 error "no member named 'missing' in namespace 'space'"
//@ diag: 15:17 error "expected namespace name"
//@ diag: 16:22 error "expected namespace name"
//@ diag: 17:13 error "out-of-line definition of 'absent' does not match any declaration in namespace 'space'"
//@ diag: 18:18 error "field designator 'nope' does not refer to any field in type 'space::Rec'"
namespace space { struct Rec { int x; }; }
using space::missing;
using namespace nowhere;
namespace a = space::gone;
void space::absent() {}
space::Rec r = {.nope = 1};
