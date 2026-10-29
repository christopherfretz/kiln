// [meta.unary.prop.query]: __array_rank and __array_extent, positive and
// negative (spec/sema-templates.md §9.4).
//@ strict-diags
static_assert(__array_rank(int) == 0 && __array_rank(int[2]) == 1 && __array_rank(int[][3][4]) == 3);
static_assert(__array_rank(int *) == 0 && __array_rank(int (&)[2]) == 0);
static_assert(__array_extent(int[2][3], 0) == 2 && __array_extent(int[2][3], 1) == 3);
static_assert(__array_extent(int[2][3], 2) == 0 && __array_extent(int[], 0) == 0 && __array_extent(int, 0) == 0);
static_assert(__is_same(decltype(__array_rank(int)), decltype(sizeof(int))));
