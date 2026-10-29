// [meta.trans.ptr]: __remove_pointer and __add_pointer
// (spec/sema-templates.md §9.2).
//@ strict-diags
static_assert(__is_same(__remove_pointer(int *), int));
static_assert(__is_same(__remove_pointer(const int *const), const int));
static_assert(__is_same(__remove_pointer(int **), int *));
static_assert(__is_same(__remove_pointer(int), int));
static_assert(__is_same(__remove_pointer(int *&), int *&));
static_assert(__is_same(__add_pointer(int), int *));
static_assert(__is_same(__add_pointer(int &), int *));
static_assert(__is_same(__add_pointer(const int &&), const int *));
static_assert(__is_same(__add_pointer(void), void *));
static_assert(__is_same(__add_pointer(int(int)), int (*)(int)));
static_assert(!__is_same(__add_pointer(int *), int *));
