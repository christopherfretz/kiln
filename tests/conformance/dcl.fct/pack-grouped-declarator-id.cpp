// [dcl.decl.general]/4, [dcl.fct]/23: `(...x)` groups the declarator-id of
// a function parameter pack, so `int (...f)()[3]` declares a pack `f` of
// function type; its erroneous return type recovers to `int` and the pack
// check reports `int ()`. clang 19 read `(...` as an abstract function
// declarator instead (requires clang>=22).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:19 error "function cannot return array type 'int[3]'"
//@ diag: 13:14 error "type 'int ()' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 14:20 error "function cannot return array type 'int[2]'"
//@ diag: 14:14 error "type 'int (int)' of function parameter pack does not contain any unexpanded parameter packs"
void g2(int (...f)()[3]);
void g8(int (... x)(int)[2]);
