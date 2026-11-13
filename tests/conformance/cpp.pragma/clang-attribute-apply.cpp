// [cpp.pragma]: '#pragma clang attribute push (attr, apply_to = rules)'
// gives the attribute to every later declaration a rule selects until the
// pop (clang ActOnPragmaAttributeAttribute, AddPragmaAttributes): member
// functions only, global variables but not locals or parameters, function
// types through a pointer, unions excluded from records. WP-1399.
//@ expect: ir
#pragma clang attribute push (__attribute__((cold)), apply_to = function)
inline int helper() { return 1; }
int use_helper() { return helper(); }
#pragma clang attribute pop

#pragma clang attribute push (__attribute__((annotate("tag"))), apply_to = any(variable(is_global), function(is_member)))
int global_var = 1;
struct T {
    int member() { return 2; }
};
int free_fn(int param) {
    int local = param;
    return local + T().member();
}
#pragma clang attribute pop

#pragma clang attribute push (__attribute__((annotate("fnty"))), apply_to = hasType(functionType))
int typed();
int (*fp)() = typed;
int not_fn = 3;
#pragma clang attribute pop

#pragma clang attribute push (__attribute__((annotate("rec"))), apply_to = variable(is_parameter))
int with_param(int p) { return p + typed(); }
#pragma clang attribute pop
