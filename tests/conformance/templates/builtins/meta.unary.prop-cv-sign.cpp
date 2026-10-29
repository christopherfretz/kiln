// [meta.unary.prop]: cv-qualification, signedness and array-bound
// predicates, positive and negative (spec/sema-templates.md §9.1).
//@ strict-diags
enum E : int { e0 };
enum class EC { a };
struct S {};

static_assert(__is_const(const int) && __is_const(const volatile S) && __is_const(int *const));
static_assert(!__is_const(int) && !__is_const(const int *) && !__is_const(const int &));
static_assert(__is_const(const int[2]));
static_assert(__is_volatile(volatile int) && __is_volatile(volatile int[2]));
static_assert(!__is_volatile(int) && !__is_volatile(volatile int *));
static_assert(__is_signed(int) && __is_signed(signed char) && __is_signed(double) && __is_signed(const long));
static_assert(!__is_signed(unsigned) && !__is_signed(bool) && !__is_signed(E) && !__is_signed(int &));
static_assert(__is_unsigned(unsigned) && __is_unsigned(bool) && __is_unsigned(unsigned long long));
static_assert(!__is_unsigned(int) && !__is_unsigned(float) && !__is_unsigned(E) && !__is_unsigned(S));
static_assert(__is_bounded_array(int[2]) && __is_bounded_array(S[1][3]));
static_assert(!__is_bounded_array(int[]) && !__is_bounded_array(int *) && !__is_bounded_array(int (&)[2]));
static_assert(__is_unbounded_array(int[]) && __is_unbounded_array(int[][2]));
static_assert(!__is_unbounded_array(int[2]) && !__is_unbounded_array(int));
static_assert(__is_scoped_enum(EC) && __is_scoped_enum(const EC));
static_assert(!__is_scoped_enum(E) && !__is_scoped_enum(int) && !__is_scoped_enum(S));
