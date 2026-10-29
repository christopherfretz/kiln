// [meta.trans.other]: __decay, __remove_cvref and __underlying_type
// (spec/sema-templates.md §9.2).
//@ strict-diags
enum E : short { e0 };
enum class F { f0 };
enum G { g0 = -1 };
struct S {};
static_assert(__is_same(__decay(const int &), int));
static_assert(__is_same(__decay(int[3]), int *));
static_assert(__is_same(__decay(const int (&)[3]), const int *));
static_assert(__is_same(__decay(int(int)), int (*)(int)));
static_assert(__is_same(__decay(const volatile S), S));
static_assert(__is_same(__decay(int *const), int *));
static_assert(__is_same(__remove_cvref(const int &), int));
static_assert(__is_same(__remove_cvref(volatile S &&), S));
static_assert(__is_same(__remove_cvref(const int[2]), int[2]));
static_assert(__is_same(__remove_cvref(const int *), const int *));
static_assert(__is_same(__underlying_type(E), short));
static_assert(__is_same(__underlying_type(F), int));
static_assert(__is_same(__underlying_type(G), int));
static_assert(!__is_same(__decay(int[3]), int[3]));
