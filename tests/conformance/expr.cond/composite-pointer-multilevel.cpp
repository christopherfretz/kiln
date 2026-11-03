// [expr.cond]/7.3, [expr.type]/4.6, [conv.qual]/3: the operands of a
// conditional expression that are similar pointers (or pointers to
// members) of any depth are converted to their cv-combined type: the
// union of the cv-qualifiers at each level, with `const` added to every
// level above one that differs. `int **` and `const int **` give
// `const int *const *`; array levels combine their element qualifiers.
struct A { int m; };
int **pp; const int **cpp; int *const *pcp;
volatile int **vpp; int ***ppp; const int ***cppp; int *volatile *pvp;
int A::**mpp; const int A::**cmpp;
int (*pa)[3]; const int (*cpa)[3];
int *(*pap)[2]; const int *(*cpap)[2];
static_assert(__is_same(decltype(true ? pp : cpp), const int *const *));
static_assert(__is_same(decltype(true ? cpp : pp), const int *const *));
static_assert(__is_same(decltype(true ? pp : pcp), int *const *));
static_assert(__is_same(decltype(true ? cpp : vpp), const volatile int *const *));
static_assert(__is_same(decltype(true ? ppp : cppp), const int *const *const *));
static_assert(__is_same(decltype(true ? pvp : cpp), const int *const volatile *));
static_assert(__is_same(decltype(true ? mpp : cmpp), const int A::*const *));
static_assert(__is_same(decltype(true ? pa : cpa), const int (*)[3]));
static_assert(__is_same(decltype(true ? pap : cpap), const int *const (*)[2]));
static_assert(__is_same(decltype(true ? pp : nullptr), int **));
constexpr int i = 0;
constexpr const int *ci = &i;
constexpr const int *const *pci = &ci;
constexpr int *pz = nullptr;
constexpr int *const *ppz = &pz;
static_assert((true ? pci : ppz) == pci);
static_assert((false ? pci : ppz) == ppz);
