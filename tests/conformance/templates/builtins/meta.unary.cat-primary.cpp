// [meta.unary.cat]: the primary type category predicates, positive and
// negative (spec/sema-templates.md §9.1).
//@ strict-diags
struct S { void f(); };
union U { int i; };
enum E { e0 };
enum class EC { a };

static_assert(__is_void(void) && __is_void(const volatile void));
static_assert(!__is_void(int) && !__is_void(void *));
static_assert(__is_nullptr(decltype(nullptr)) && __is_nullptr(const decltype(nullptr)));
static_assert(!__is_nullptr(void *) && !__is_nullptr(int));
static_assert(__is_integral(int) && __is_integral(bool) && __is_integral(char8_t));
static_assert(__is_integral(const unsigned long) && __is_integral(wchar_t));
static_assert(!__is_integral(float) && !__is_integral(E) && !__is_integral(int &));
static_assert(__is_floating_point(double) && __is_floating_point(const long double));
static_assert(!__is_floating_point(int) && !__is_floating_point(double &));
static_assert(__is_array(int[2]) && __is_array(int[]) && __is_array(S[1][2]));
static_assert(!__is_array(int *) && !__is_array(int (&)[2]));
static_assert(__is_pointer(int *) && __is_pointer(void (*)()) && __is_pointer(S *const));
static_assert(!__is_pointer(int S::*) && !__is_pointer(decltype(nullptr)) && !__is_pointer(int *&));
static_assert(__is_lvalue_reference(int &) && __is_lvalue_reference(void (&)()));
static_assert(!__is_lvalue_reference(int &&) && !__is_lvalue_reference(int));
static_assert(__is_rvalue_reference(int &&) && __is_rvalue_reference(S &&));
static_assert(!__is_rvalue_reference(int &) && !__is_rvalue_reference(int));
static_assert(__is_member_object_pointer(int S::*) && __is_member_object_pointer(int S::*const));
static_assert(!__is_member_object_pointer(void (S::*)()) && !__is_member_object_pointer(int *));
static_assert(__is_member_function_pointer(void (S::*)()) && __is_member_function_pointer(int (S::*)(int) const));
static_assert(!__is_member_function_pointer(int S::*) && !__is_member_function_pointer(void (*)()));
static_assert(__is_enum(E) && __is_enum(EC) && __is_enum(const E));
static_assert(!__is_enum(int) && !__is_enum(S) && !__is_enum(E &));
static_assert(__is_union(U) && __is_union(const U));
static_assert(!__is_union(S) && !__is_union(int));
static_assert(__is_class(S) && __is_class(volatile S));
static_assert(!__is_class(U) && !__is_class(E) && !__is_class(S *));
static_assert(__is_function(void()) && __is_function(int(int, ...) noexcept));
static_assert(!__is_function(void (*)()) && !__is_function(void (&)()) && !__is_function(S));
