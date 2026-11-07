// __builtin_offsetof names a member found by class member lookup, so a
// member of a (non-virtual) base class too; the base subobject's offset
// is added ([class.member.lookup], [support.types.layout]).
//@ expect: run
//@ args: -Wno-invalid-offsetof
//@ strict-diags

struct A { char a; int ai; };
struct B { long b; struct { short s; int t; } nested; };
struct C : A, B { int c; };
struct D : C { char d; };
struct E final : D {
  static constexpr unsigned long inside() { return __builtin_offsetof(E, nested.t); }
};

// The layout as the compiler computed it, through real member addresses.
template <class T, class M>
unsigned long real(T& obj, M& m) {
  return (unsigned long)((const char*)&m - (const char*)&obj);
}

static_assert(__builtin_offsetof(C, ai) == __builtin_offsetof(A, ai), "");
static_assert(__builtin_offsetof(C, b) >= sizeof(A), "");
static_assert(__builtin_offsetof(D, nested.s) == __builtin_offsetof(C, nested.s), "");
static_assert(E::inside() == __builtin_offsetof(D, nested.t), "");

int main() {
  E e;
  if (__builtin_offsetof(E, a) != real(e, e.a)) return 1;
  if (__builtin_offsetof(E, ai) != real(e, e.ai)) return 2;
  if (__builtin_offsetof(E, b) != real(e, e.b)) return 3;
  if (__builtin_offsetof(E, nested.t) != real(e, e.nested.t)) return 4;
  if (__builtin_offsetof(E, c) != real(e, e.c)) return 5;
  if (__builtin_offsetof(E, d) != real(e, e.d)) return 6;
  if (E::inside() != real(e, e.nested.t)) return 7;
  return 0;
}
