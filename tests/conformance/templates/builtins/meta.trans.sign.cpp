// [meta.trans.sign]: __make_signed and __make_unsigned, including enums
// and cv-qualifiers (spec/sema-templates.md §9.2).
//@ strict-diags
enum E : unsigned short { e0 };
enum class B : short { b0 };
static_assert(__is_same(__make_signed(unsigned), int));
static_assert(__is_same(__make_signed(int), int));
static_assert(__is_same(__make_signed(const unsigned long), const long));
static_assert(__is_same(__make_signed(volatile unsigned char), volatile signed char));
static_assert(__is_same(__make_signed(char), signed char));
static_assert(__is_same(__make_signed(E), short));
static_assert(__is_same(__make_unsigned(int), unsigned));
static_assert(__is_same(__make_unsigned(signed char), unsigned char));
static_assert(__is_same(__make_unsigned(const long long), const unsigned long long));
static_assert(__is_same(__make_unsigned(B), unsigned short));
static_assert(__is_same(__make_unsigned(char), unsigned char));
static_assert(!__is_same(__make_unsigned(int), int));
