// [dcl.type.elab]/4 / [class.pre]/3: the class-head-name of a qualified class definition names
// a class previously declared in the qualifier's scope (clang's `err_not_tag_in_scope`)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "no struct named 'Nope' in 'NT'"
//@ diag: 11:11 error "no class named 'Q' in 'NT'"
//@ diag: 12:31 error "redefinition of 'R' as different kind of symbol"
//@ diag: 9:20 note "previous definition is here"
struct NT { struct R; };
struct NT::Nope {};
class NT::Q {};
template <class T> struct NT::R {};
