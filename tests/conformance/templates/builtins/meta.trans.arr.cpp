// [meta.trans.arr]: __remove_extent and __remove_all_extents
// (spec/sema-templates.md §9.2).
//@ strict-diags
static_assert(__is_same(__remove_extent(int[2]), int));
static_assert(__is_same(__remove_extent(int[2][3]), int[3]));
static_assert(__is_same(__remove_extent(int[][3]), int[3]));
static_assert(__is_same(__remove_extent(const int[2]), const int));
static_assert(__is_same(__remove_extent(int), int));
static_assert(__is_same(__remove_extent(int (&)[2]), int (&)[2]));
static_assert(__is_same(__remove_all_extents(int[2][3][4]), int));
static_assert(__is_same(__remove_all_extents(const int[][3]), const int));
static_assert(__is_same(__remove_all_extents(int *), int *));
static_assert(!__is_same(__remove_extent(int[2][3]), int));
