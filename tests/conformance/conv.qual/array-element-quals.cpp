// [conv.qual]/3 with [basic.type.qualifier]/3: an array type is
// cv-qualified as its elements are, so the array is not a qualification
// level of its own. Adding volatile (or const volatile) to the elements of
// a pointed-to array needs no const above it (std::span<volatile int> from
// int*: is_convertible_v<int (*)[], volatile int (*)[]>).
//@ expect: compile-ok
static_assert(__is_convertible(int (*)[], volatile int (*)[]));
static_assert(__is_convertible(int (*)[], const int (*)[]));
static_assert(__is_convertible(int (*)[3], const volatile int (*)[3]));
static_assert(__is_convertible(int (*)[3], volatile int (*)[]));
static_assert(__is_convertible(int (*)[2][3], volatile int (*)[2][3]));
static_assert(__is_convertible(int **, const int *const *));
// Below a pointer that is not const, adding a qualifier is still refused.
static_assert(!__is_convertible(int **, volatile int **));
static_assert(!__is_convertible(int *(*)[3], volatile int *(*)[3]));
static_assert(__is_convertible(int *(*)[3], volatile int *const (*)[3]));
static_assert(!__is_convertible(volatile int (*)[], int (*)[]));
volatile int (*f(int (*p)[4]))[4] { return p; }
int main() {}
