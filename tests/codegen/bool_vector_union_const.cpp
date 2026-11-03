// [WP-438] A union or struct global with a `bool` ext-vector member
// initialized by a constant (clang ext_vector_type; [class.union] p2,
// [expr.const]; clang ConstExprEmitter) is a folded constant with no
// `__cxx_global_var_init`: the lane stores of the member's construction
// go through the union like a bit-field's (spec/oven.md "Bool vectors").
typedef bool b8 __attribute__((ext_vector_type(8)));
union U { b8 v; unsigned char c; };
struct S { int i; U u; };

constexpr U u = {(b8)true};
U u1 = {(b8){true, false, true}};
S s = {1, {(b8){false, true}}};
U arr[2] = {{(b8)true}, {.c = 7}};
const void *p = &u;
