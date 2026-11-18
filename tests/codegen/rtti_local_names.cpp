// RTTI names of unit-local types (spec/abi.md "RTTI names of unit-local
// types", WP-1651): kiln writes GCC's `*` in front of the `_ZTS` string
// of a type local to the unit under libstdc++; clang does not, so the
// strings are a listed difference (`RTTI_LOCAL_MARKER_CASES`) and the
// rest must equal clang's: the linkage of every `type_info` and name
// (an array or function type built from a unit-local type is `internal`,
// as clang), and clang's `$_N` names of unnamed classes without linkage
// (an unnamed struct and an unnamed enumeration at namespace scope are
// two types, two names).
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std
const std::type_info *sink[64];
int n;
#define T(x) sink[n++] = &typeid(x)
namespace {
struct A {
  struct In {};
};
enum EA { ea };
struct AP {
  virtual ~AP();
};
AP::~AP() {}
struct IncA;
} // namespace
static void sfun() {
  struct LS {};
  T(LS);
  auto l = [] {};
  T(l);
  struct { int a; } u;
  T(u);
}
inline void ifun() {
  struct LI {};
  T(LI);
  auto l = [] {};
  T(l);
}
void efun() {
  struct LE {};
  T(LE);
}
template <class U> void tf() {
  struct L {};
  T(L);
}
auto nl = [] {};
static auto snl = [] {};
struct { int a; } us;
enum { ue1 } ue;
template <class> struct Tm {};
template <int *> struct Tp {};
template <void (*)()> struct Tf {};
static int sint;
int eint;
struct Ext {};
struct DerivA : A {};
struct Inc;
struct [[gnu::visibility("hidden")]] Hid {};
void use() {
  sfun();
  ifun();
  efun();
  tf<int>();
  tf<A>();
  T(A); T(A::In); T(EA); T(AP); T(nl); T(snl); T(us); T(ue);
  T(Tm<A>); T(Tm<A *>); T(Tp<&sint>); T(Tp<&eint>); T(Tf<&sfun>); T(Tf<&efun>); T(Tm<Ext>);
  T(A *); T(int A::*); T(Ext A::*); T(A Ext::*); T(void(A)); T(A (*)()); T(A[3]); T(A *[2]);
  T(const A *); T(A const *volatile *);
  T(DerivA); T(Ext);
  T(Inc *); T(IncA *); T(int Inc::*);
  T(Hid); T(Hid *);
}
