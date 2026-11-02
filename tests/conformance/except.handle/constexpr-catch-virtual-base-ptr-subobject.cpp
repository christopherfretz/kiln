// [except.handle]/3.3 + [conv.ptr]/3 (C++26, P3068R6): a thrown "pointer
// to A" that designates a base class subobject A of a most derived object
// D is caught by a handler of type "pointer to cv V" for V a virtual base
// of A: the caught pointer designates the V subobject of D (the most
// derived object's virtual base, whose offset A's own layout does not
// give). The objects are static (a class with a virtual base has no
// constexpr constructor): pointers caught during constant evaluation
// initialize constexpr variables that main compares with the run-time
// conversions.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 1 1
//@ stdout: 1 1 1 1 1
extern "C" int printf(const char *, ...);

struct V { int v; };
struct W : V { int w; };
struct A : virtual V { int a; };
struct B : virtual V { int b; };
struct D : A, B { int d; };
D d;

// Through the second base, whose own layout puts V elsewhere than D's.
constexpr V *from_b() {
  try {
    throw static_cast<B *>(&d);
  } catch (V *p) {
    return p;
  }
}
constexpr const V *from_a() {
  try {
    throw static_cast<A *>(&d);
  } catch (const V *p) {
    return p;
  }
}

// A non-virtual base (V) of a virtual base (W) of the thrown subobject's
// class (C), in an array element that is itself a member.
struct C : virtual W { int c; };
struct E : C { int e; };
struct F : E { int f; };
struct Holder { int pad; F fs[2]; };
Holder h;

constexpr V *from_c_in_member() {
  try {
    throw static_cast<C *>(&h.fs[1]);
  } catch (V *p) {
    return p;
  }
}
constexpr W *w_from_e() {
  try {
    throw static_cast<E *>(&h.fs[0]);
  } catch (W *p) {
    return p;
  }
}

// A null pointer to a base subobject converts to a null V*.
constexpr bool null_from_b() {
  try {
    throw static_cast<B *>(nullptr);
  } catch (V *p) {
    return p == nullptr;
  }
}

constexpr bool same_vbase() { return from_a() == from_b(); }
static_assert(same_vbase());
static_assert(null_from_b());
static_assert(from_c_in_member() != static_cast<V *>(w_from_e()));

constexpr V *ce_b = from_b();
constexpr const V *ce_a = from_a();
constexpr V *ce_c = from_c_in_member();
constexpr W *ce_w = w_from_e();

int main() {
  printf("%d %d %d %d %d\n", same_vbase(), null_from_b(), ce_b == static_cast<V *>(&d),
         ce_a == static_cast<V *>(&d), ce_c == static_cast<V *>(&h.fs[1]));
  B *bp = &d;
  C *cp = &h.fs[1];
  E *ep = &h.fs[0];
  printf("%d %d %d %d %d\n", ce_b == static_cast<V *>(bp), ce_c == static_cast<V *>(cp),
         ce_w == static_cast<W *>(ep), ce_w == static_cast<W *>(&h.fs[0]),
         ce_c != static_cast<V *>(&h.fs[0]));
}
