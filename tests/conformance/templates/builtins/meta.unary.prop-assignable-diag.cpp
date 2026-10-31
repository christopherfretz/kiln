// [meta.unary.prop]/3 is_assignable: T and U shall be complete types, cv
// void, or arrays of unknown bound; clang diagnoses an incomplete operand
// at the operand.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:26 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 10:8 note "forward declaration of 'Inc'"
//@ diag: 13:32 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 10:8 note "forward declaration of 'Inc'"
struct Inc;
bool b = __is_assignable(Inc, int);
bool c = __is_assignable(int&, Inc&);
bool d = __is_assignable(int&, Inc);
