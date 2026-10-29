// [dcl.mptr]/1-2: `nested-name-specifier * cv` declares a pointer to member of
// the named class, in every declarator position, with cv after the `*`;
// function member pointers keep the function's cv- and ref-qualifiers.
// [dcl.mptr]/4 (incomplete class allowed), [conv.mem]/2, [basic.compound].
struct C {
  int m;
  int arr[3];
  void f(int) const;
  int g() &;
  int h() &&;
  static int s;
};
struct D : C { int n; };
union U { int a; float b; };
struct Inc;
namespace N { struct E { int x; struct In { int y; }; }; }
typedef C Alias;

int C::*p1 = &C::m;
int C::* const p2 = &C::m;
int C::* volatile const p3 = nullptr;
void (C::*p4)(int) const = &C::f;
int (C::*p5)() & = &C::g;
int (C::*p6)() && = &C::h;
int (C::*p7)[3] = &C::arr;
int (C::*(p8)) = &C::m;
int U::*p9 = &U::a;
int Inc::*p10 = nullptr;
int N::E::*p11 = &N::E::x;
int N::E::In::*p12 = &N::E::In::y;
int ::N::E::*p13 = &N::E::x;
int Alias::*p14 = &C::m;
int D::*p15 = &C::m;
int C::*C::*p16 = nullptr;
int C::*arr_of_mp[2] = {&C::m, nullptr};
int C::**ptr_to_mp = &p1;
int C::*&ref_to_mp = p1;
const int C::*p17 = &C::m;

typedef int C::*DataPtr;
using FnPtr = void (C::*)(int) const;
DataPtr q1 = p1;
FnPtr q2 = p4;

void params(int C::*, void (C::*)(int) const, int (C::*)[3], int C::*named);
int (C::*returns_mp())[3];
void (C::*returns_fmp(int))(int) const;

static_assert(sizeof(int C::*) == sizeof(long));
static_assert(sizeof(void (C::*)(int) const) == 2 * sizeof(void *));
static_assert(alignof(int C::*) == alignof(long));
static_assert(sizeof(DataPtr) == sizeof(long));

int C::*casts() {
  auto a = (int C::*)nullptr;
  auto b = static_cast<int D::*>(&C::m);
  (void)b;
  int C::**c = new (int C::*)(nullptr);
  int C::**d = new int C::*;
  delete c;
  delete d;
  return a;
}

int use(C c, C *pc) {
  return c.*p1 + pc->*p2 + (c.*p7)[1];
}

template <class T> struct Holder {
  int T::*field;
  using Fn = void (T::*)(int) const;
  typename T::In T::*nested;
};
template <class T> int T::*tpl_var = nullptr;
template <class T> void tpl_fn(int T::*, void (T::*)());
