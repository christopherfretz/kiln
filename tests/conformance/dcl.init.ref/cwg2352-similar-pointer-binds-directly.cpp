// [dcl.init.ref]/4-5 as amended by CWG2352: `cv1 T1` is reference-
// compatible with `cv2 T2` when a pointer to `cv2 T2` converts to a
// pointer to `cv1 T1` by a qualification conversion ([conv.qual]), so a
// `const int *const &` initialized by an `int *` lvalue (or xvalue)
// binds to that object directly, with no temporary: the reference and
// the pointer have the same address, also in a constant expression.
// Arrays of similar pointers and pointers to members bind the same way;
// a prvalue still materializes a temporary.
//@ expect: compile-ok
struct S {
  int m;
};
template <class T> struct Vec {
  T *b = nullptr;
  constexpr const T *const &begin() const { return b; }
};
constexpr bool addr_identity() {
  int x = 1;
  int *p = &x;
  const int *const &r = p;
  const volatile int *const &rv = p;
  int *a[2] = {p, p};
  const int *const (&ra)[2] = a;
  int S::*mp = &S::m;
  const int S::*const &rmp = mp;
  const int *const &rs = static_cast<const int *const &>(p);
  const int *const &rx = static_cast<int *&&>(p);
  Vec<int> v;
  v.b = p;
  bool ok = &r == &p && (const void *)&rv == (const void *)&p && &ra[1] == &a[1] &&
            ra[0] == &x && &rmp == &mp && &rs == &p && &rx == &p && &v.begin() == &v.b;
  p = nullptr;
  return ok && r == nullptr && *ra[1] == 1;
}
static_assert(addr_identity());
int *gp;
const int *const &gr = gp;
static_assert(&gr == &gp);
int **gpp;
const int *const *const &grr = gpp;
static_assert(&grr == &gpp);
// A prvalue initializer still binds a temporary.
const int *const *const &r3 = (int **)nullptr;
