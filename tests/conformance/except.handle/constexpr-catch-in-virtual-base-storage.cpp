// [except.handle]/3.3, [conv.ptr]/3, [class.mi]/4 (C++26, P3068R6 and
// P3533R2): a thrown pointer to a subobject that lives inside the storage of
// an enclosing object's virtual base is caught by a handler for a base of
// that subobject's type. The objects are constexpr locals (P3533 lets a
// class with virtual bases be constructed during constant evaluation), so
// the catch must find the complete object through the virtual-base storage:
// the thrown pointer's class is itself a virtual base of the complete
// object, or a non-virtual base of one, also in an array element member.
// main repeats every throw at run time.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068 or P3533
//@ expect: run
//@ strict-diags
//@ stdout: 19 1 1 46 1
//@ stdout: 19 1 1 46 1
extern "C" int printf(const char *, ...);

struct V { int v = 1; };
struct A : virtual V { int a = 2; };
struct B : virtual V { int b = 3; };
struct D : virtual A, virtual B { int d = 4; };
struct Outer { int pad = 0; D ds[2]; };

// B is a virtual base of D; V is a virtual base of B whose offset only the
// complete object's layout gives.
constexpr int from_vbase_in_member() {
  Outer o;
  o.ds[1].v = 9;
  try {
    throw static_cast<B *>(&o.ds[1]);
  } catch (V *p) {
    return p->v + (p == static_cast<V *>(&o.ds[1])) * 10;
  }
}

// W is a non-virtual base of X, and X a virtual base of Y: the thrown W
// subobject sits inside Y's virtual-base storage.
struct W : virtual V { int w = 5; };
struct X : W { int x = 6; };
struct Y : virtual X, virtual A { int y = 7; };
constexpr bool from_nv_base_of_vbase() {
  Y y;
  try {
    throw static_cast<W *>(&y);
  } catch (const V *p) {
    return p == static_cast<const V *>(&y) && p == static_cast<A *>(&y);
  }
}

// The same through a heap object.
constexpr bool from_heap() {
  Y *y = new Y;
  bool ok = false;
  try {
    throw static_cast<X *>(y);
  } catch (W *p) {
    ok = p == static_cast<W *>(y) && p->w == 5;
  }
  delete y;
  return ok;
}

// The caught pointer reads and writes the shared virtual base.
constexpr int write_through() {
  Y y;
  try {
    throw static_cast<W *>(&y);
  } catch (V *p) {
    p->v = 40;
  }
  return y.v + static_cast<A &>(y).a + y.w - 1;
}

constexpr bool null_in_vbase() {
  try {
    throw static_cast<W *>(nullptr);
  } catch (V *p) {
    return p == nullptr;
  }
}

static_assert(from_vbase_in_member() == 19);
static_assert(from_nv_base_of_vbase());
static_assert(from_heap());
static_assert(write_through() == 46);
static_assert(null_in_vbase());

constexpr int ce1 = from_vbase_in_member();
constexpr bool ce2 = from_nv_base_of_vbase();
constexpr bool ce3 = from_heap();
constexpr int ce4 = write_through();
constexpr bool ce5 = null_in_vbase();

int main() {
  printf("%d %d %d %d %d\n", ce1, ce2, ce3, ce4, ce5);
  printf("%d %d %d %d %d\n", from_vbase_in_member(), from_nv_base_of_vbase(), from_heap(),
         write_through(), null_in_vbase());
}
