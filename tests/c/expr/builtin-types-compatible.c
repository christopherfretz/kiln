// C17 6.2.7 (compatible types), GNU __builtin_types_compatible_p: 1 when
// the unqualified types are compatible (top-level qualifiers, and those
// of array elements, are ignored; an enum is compatible with its
// underlying type), else 0; an integer constant expression of type int.
//@ strict-diags
#if !__has_builtin(__builtin_types_compatible_p)
#error no __builtin_types_compatible_p
#endif
enum E { A, B };
enum F { C = -1 };
typedef int T;
typedef const int CA[5];
_Static_assert(__builtin_types_compatible_p(int, const int), "");
_Static_assert(__builtin_types_compatible_p(int, volatile T), "");
_Static_assert(__builtin_types_compatible_p(const int, const volatile int), "");
_Static_assert(!__builtin_types_compatible_p(int, long), "");
_Static_assert(!__builtin_types_compatible_p(char, signed char), "");
_Static_assert(__builtin_types_compatible_p(enum E, unsigned), "");
_Static_assert(__builtin_types_compatible_p(enum F, int), "");
_Static_assert(!__builtin_types_compatible_p(enum E, enum F), "");
_Static_assert(!__builtin_types_compatible_p(int *, const int *), "");
_Static_assert(__builtin_types_compatible_p(int[], int[5]), "");
_Static_assert(!__builtin_types_compatible_p(int[4], int[5]), "");
_Static_assert(__builtin_types_compatible_p(CA, int[5]), "");
_Static_assert(__builtin_types_compatible_p(const int[2][3], int[2][3]), "");
_Static_assert(!__builtin_types_compatible_p(const int *[2], int *[2]), "");
_Static_assert(__builtin_types_compatible_p(int (*)(), int (*)(int)), "");
_Static_assert(__builtin_types_compatible_p(__typeof__(1), int), "");
_Static_assert(__builtin_types_compatible_p(struct S, struct S), "");
_Static_assert(!__builtin_types_compatible_p(void, int), "");
_Static_assert(sizeof(__builtin_types_compatible_p(int, int)) == sizeof(int), "");
int arr[__builtin_types_compatible_p(int, int) + 1];
_Static_assert(sizeof arr == 2 * sizeof(int), "");
int v = __builtin_choose_expr(__builtin_types_compatible_p(int, int), 1, (void)0);
