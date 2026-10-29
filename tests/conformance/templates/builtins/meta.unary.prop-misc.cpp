// [meta.unary.prop]: __is_literal (the older spelling of
// __is_literal_type) and clang's __is_complete_type, positive and negative
// (spec/sema-templates.md §9.1).
//@ strict-diags
struct I;
struct S { int x; };
struct VDtor { virtual ~VDtor(); };
static_assert(__is_literal(int) && __is_literal(S) && __is_literal(int[2]));
static_assert(!__is_literal(void()) && !__is_literal(VDtor));
static_assert(__is_complete_type(S) && __is_complete_type(int) && __is_complete_type(int &));
static_assert(!__is_complete_type(I) && !__is_complete_type(void) && !__is_complete_type(int[]));
