// [meta.unary.comp]: the composite type category predicates, positive and
// negative (spec/sema-templates.md §9.1).
//@ strict-diags
struct S { int m; };
enum E { e0 };

static_assert(__is_reference(int &) && __is_reference(int &&));
static_assert(!__is_reference(int) && !__is_reference(int *));
static_assert(__is_arithmetic(int) && __is_arithmetic(double) && __is_arithmetic(bool));
static_assert(!__is_arithmetic(E) && !__is_arithmetic(int *) && !__is_arithmetic(int &));
static_assert(__is_fundamental(void) && __is_fundamental(decltype(nullptr)) && __is_fundamental(float));
static_assert(!__is_fundamental(E) && !__is_fundamental(S) && !__is_fundamental(int *));
static_assert(__is_object(int) && __is_object(S) && __is_object(int[]) && __is_object(int *));
static_assert(!__is_object(void) && !__is_object(int &) && !__is_object(void()));
static_assert(__is_scalar(int) && __is_scalar(E) && __is_scalar(int *) && __is_scalar(int S::*));
static_assert(__is_scalar(decltype(nullptr)));
static_assert(!__is_scalar(S) && !__is_scalar(int[1]) && !__is_scalar(int &) && !__is_scalar(void));
static_assert(__is_compound(int *) && __is_compound(S) && __is_compound(E) && __is_compound(int &));
static_assert(__is_compound(void()) && __is_compound(int[2]));
static_assert(!__is_compound(int) && !__is_compound(void) && !__is_compound(decltype(nullptr)));
static_assert(__is_member_pointer(int S::*) && __is_member_pointer(void (S::*)()));
static_assert(!__is_member_pointer(int *) && !__is_member_pointer(S));
static_assert(__is_referenceable(int) && __is_referenceable(void()) && __is_referenceable(int &));
static_assert(!__is_referenceable(void) && !__is_referenceable(void() const));
