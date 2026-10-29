// __builtin_bit_cast (spec/sema-templates.md §9.5, [bit.cast]): the
// object representation of the operand reinterpreted as the target type,
// for scalars and classes. (Calling a class template's member function
// needs template member codegen, WP-079; the Oven side is unowned, §9.5.)
//@ expect: run
//@ strict-diags
struct P { float a; int b; };
struct Q { int a; float b; };
unsigned u(float f) { return __builtin_bit_cast(unsigned, f); }
float fl(unsigned x) { return __builtin_bit_cast(float, x); }
Q pq(const P *p) { return __builtin_bit_cast(Q, *p); }
template <class T> struct Bits { typedef decltype(__builtin_bit_cast(unsigned, T())) type; };
static_assert(__is_same(Bits<float>::type, unsigned));
static_assert(__is_same(decltype(__builtin_bit_cast(const int, 1.0f)), int));
extern P ep;
static_assert(__is_same(decltype(__builtin_bit_cast(Q, ep)), Q));
int main() {
  if (u(1.0f) != 0x3f800000u)
    return 1;
  if (fl(0x40000000u) != 2.0f)
    return 2;
  P p{1.0f, 7};
  Q q = pq(&p);
  if (q.a != 0x3f800000 || __builtin_bit_cast(int, q.b) != 7)
    return 3;
  bool b = __builtin_bit_cast(bool, (unsigned char)1);
  if (!b)
    return 5;
  return 0;
}
