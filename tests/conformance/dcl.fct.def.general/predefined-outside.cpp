// [dcl.fct.def.general]/8: `__func__` is only defined in a function body;
// clang (and kiln) accept it elsewhere with a warning, naming the
// translation unit: "top level" for `__PRETTY_FUNCTION__`, "" otherwise. A
// default member initializer is not in a function.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:19 warning "predefined identifier is only valid inside function"
//@ diag: 12:20 warning "predefined identifier is only valid inside function"
//@ diag: 13:28 warning "predefined identifier is only valid inside function"
//@ diag: 15:22 warning "predefined identifier is only valid inside function"
const char* top = __PRETTY_FUNCTION__;
const char* top2 = __func__;
struct D { const char* m = __FUNCTION__; };
static_assert(sizeof(top) == sizeof(void*));
static_assert(sizeof(__PRETTY_FUNCTION__) == 10);
void f() { (void)__func__; }
