// RTTI names of unit-local types under libc++ (WP-1818): libc++ compares
// `type_info` objects by the address of their names, so kiln writes no
// `*` marker in front of a `_ZTS` string (GCC's libstdc++ convention,
// spec/abi.md "RTTI names of unit-local types", WP-1651) and the IR is
// clang 22's and 19's exactly on both Linux targets: the cases of
// rtti_local_names.cpp, without its listed difference.
//@ args: -stdlib=libc++
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
