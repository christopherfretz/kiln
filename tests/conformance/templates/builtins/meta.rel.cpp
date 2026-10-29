// [meta.rel]: __is_same and __is_base_of, positive and negative
// (spec/sema-templates.md §9.1).
//@ strict-diags
struct B {};
struct D : B {};
struct P : private B {};
struct V : virtual B {};
struct DD : D {};
union U { int i; };
typedef int Int;

static_assert(__is_same(int, Int) && __is_same(const int *, const Int *) && __is_same(D, D));
static_assert(!__is_same(int, const int) && !__is_same(int, int &) && !__is_same(B, D) && !__is_same(int[], int[1]));
static_assert(__is_base_of(B, D) && __is_base_of(B, DD) && __is_base_of(B, P) && __is_base_of(B, V));
static_assert(__is_base_of(B, B) && __is_base_of(const B, volatile D));
static_assert(!__is_base_of(D, B) && !__is_base_of(int, int) && !__is_base_of(U, U) && !__is_base_of(B &, D &));
