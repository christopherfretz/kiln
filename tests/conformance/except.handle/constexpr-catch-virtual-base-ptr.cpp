// [except.handle]/3.3 + [conv.ptr]/3 (C++26, P3068R6): a handler of type
// "pointer to cv V" matches a thrown "pointer to D" when V is an
// unambiguous public base of D, virtual or not; the caught pointer
// designates D's V subobject (through the virtual base's offset). An
// ambiguous base does not match. The objects are static (a class with a
// virtual base has no constexpr constructor), so only addresses are
// compared: pointers caught during constant evaluation initialize
// constexpr variables that main compares with the run-time conversions.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 1 1
//@ stdout: 1 1 1
extern "C" int printf(const char *, ...);

struct V { int v; };
struct A : virtual V { int a; };
struct B : virtual V { int b; };
struct D : A, B { int d; };
D d;

constexpr V *caught_vbase() {
  try {
    throw &d;
  } catch (V *p) {
    return p;
  }
}

constexpr const volatile V *caught_cv_vbase() {
  try {
    throw &d;
  } catch (const volatile V *p) {
    return p;
  }
}

// The non-virtual bases of a class with a virtual base match too.
constexpr B *caught_base() {
  try {
    throw &d;
  } catch (B *p) {
    return p;
  }
}

constexpr bool through_vbase() {
  return caught_vbase() && static_cast<void *>(caught_vbase()) != static_cast<void *>(&d);
}
constexpr bool through_cv_vbase() { return caught_cv_vbase() == caught_vbase(); }
constexpr bool non_virtual_base() { return caught_base() == static_cast<B *>(&d); }

struct X { int x; };
struct P : X {};
struct Q : X {};
struct R : P, Q {};
R r;

// X is an ambiguous base of R: the catch-all handler is selected.
constexpr bool ambiguous() {
  try {
    throw &r;
  } catch (X *) {
    return false;
  } catch (...) {
    return true;
  }
}

// A null pointer converts to a null pointer to the virtual base.
constexpr bool null_to_vbase() {
  try {
    throw static_cast<D *>(nullptr);
  } catch (V *p) {
    return p == nullptr;
  }
}

static_assert(through_vbase());
static_assert(through_cv_vbase());
static_assert(non_virtual_base());
static_assert(ambiguous());
static_assert(null_to_vbase());

constexpr V *ce_vbase = caught_vbase();
constexpr const volatile V *ce_cv_vbase = caught_cv_vbase();
constexpr B *ce_base = caught_base();

int main() {
  printf("%d %d %d %d %d\n", through_vbase(), through_cv_vbase(), non_virtual_base(), ambiguous(),
         null_to_vbase());
  V *rt = &d;
  printf("%d %d %d\n", ce_vbase == rt, ce_cv_vbase == rt, ce_base == static_cast<B *>(&d));
}
