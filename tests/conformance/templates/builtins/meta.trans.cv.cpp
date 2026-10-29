// [meta.trans.cv]: __remove_const, __remove_volatile, __remove_cv, and
// clang's __remove_restrict (spec/sema-templates.md §9.2).
//@ strict-diags
static_assert(__is_same(__remove_const(const int), int));
static_assert(__is_same(__remove_const(const volatile int), volatile int));
static_assert(__is_same(__remove_const(const int *), const int *));
static_assert(__is_same(__remove_const(int *const), int *));
static_assert(__is_same(__remove_const(const int[2]), int[2]));
static_assert(__is_same(__remove_const(const int &), const int &));
static_assert(__is_same(__remove_volatile(const volatile int), const int));
static_assert(__is_same(__remove_volatile(volatile int[3]), int[3]));
static_assert(__is_same(__remove_cv(const volatile int), int));
static_assert(__is_same(__remove_cv(const volatile int *), const volatile int *));
static_assert(__is_same(__remove_cv(int), int));
static_assert(__is_same(__remove_restrict(int *__restrict), int *));
static_assert(!__is_same(__remove_cv(const int &), int &));
