// [meta.trans.ref]: __remove_reference_t, __add_lvalue_reference and
// __add_rvalue_reference, including the non-referenceable types
// (spec/sema-templates.md §9.2).
//@ strict-diags
struct S {};
static_assert(__is_same(__remove_reference_t(int &), int));
static_assert(__is_same(__remove_reference_t(const int &&), const int));
static_assert(__is_same(__remove_reference_t(int), int));
static_assert(__is_same(__remove_reference_t(int *&), int *));
static_assert(__is_same(__add_lvalue_reference(int), int &));
static_assert(__is_same(__add_lvalue_reference(int &&), int &));
static_assert(__is_same(__add_lvalue_reference(void), void));
static_assert(__is_same(__add_lvalue_reference(void()), void (&)()));
static_assert(__is_same(__add_rvalue_reference(S), S &&));
static_assert(__is_same(__add_rvalue_reference(int &), int &));
static_assert(__is_same(__add_rvalue_reference(const void), const void));
static_assert(!__is_same(__add_rvalue_reference(int), int &));
