// clang's expression traits __is_lvalue_expr / __is_rvalue_expr
// ([basic.lval]), positive and negative (spec/sema-templates.md §9). As in
// clang, an xvalue is neither: the traits test lvalue and prvalue.
//@ strict-diags
int x;
int &lref();
int &&xref();
int prv();
static_assert(__is_lvalue_expr(x) && __is_lvalue_expr(lref()) && __is_lvalue_expr(*&x));
static_assert(!__is_lvalue_expr(1) && !__is_lvalue_expr(prv()) && !__is_lvalue_expr(xref()));
static_assert(__is_rvalue_expr(1) && __is_rvalue_expr(prv()) && __is_rvalue_expr(x + 1));
static_assert(!__is_rvalue_expr(x) && !__is_rvalue_expr(lref()) && !__is_rvalue_expr(xref()));
