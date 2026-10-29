// [meta.trans.other]: __underlying_type of a non-enumeration type; an
// opaque enumeration with a fixed type is complete (spec/sema-templates.md §9.2).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "only enumeration types have underlying types"
//@ diag: 9:9 error "only enumeration types have underlying types"
struct S {};
typedef __underlying_type(int) A;
typedef __underlying_type(S) B;
enum class Ok : char;
typedef __underlying_type(Ok) C;
