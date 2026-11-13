// [expr.static.cast]/10: an integer converted to an enumeration with a
// fixed underlying type is first converted to that type, so for `bool`
// it is a boolean conversion ([conv.bool]: nonzero is `true`), not a
// truncation to one bit (clang `CK_IntegralToBoolean`). [expr.static.cast]/9,
// [conv.prom]/4: the enumeration converts back, and an unscoped one
// promotes, through `bool` (0 or 1). The shape is real code's
// `enum class Policy : bool`.
//@ expect: run
//@ stdout: 1 1 0 1 1 0
//@ stdout: 1 0 1 0 2 1
//@ stdout: 1 0 1 1
//@ stdout: 1 0 3
extern "C" int printf(const char *, ...);

enum class P : bool { NO, YES };
enum U : bool { UN, UY };
enum class Wide { A = 0, B = 2 };

__attribute__((noinline)) P from_int(int i) { return static_cast<P>(i); }
__attribute__((noinline)) P from_long(long i) { return P(i); }
__attribute__((noinline)) U from_uint(unsigned i) { return (U)i; }
__attribute__((noinline)) P from_bool(bool b) { return static_cast<P>(b); }
__attribute__((noinline)) P from_enum(Wide w) { return static_cast<P>(w); }
__attribute__((noinline)) P from_double(double d) { return static_cast<P>(d); }
__attribute__((noinline)) int to_int(P p) { return static_cast<int>(p); }
__attribute__((noinline)) bool to_bool(P p) { return static_cast<bool>(p); }
__attribute__((noinline)) int promote(U u) { return u + u; }
__attribute__((noinline)) bool implicit(U u) { return u; }
__attribute__((noinline)) double to_double(P p) { return static_cast<double>(p); }

int main(int argc, char **) {
  int two = argc + 1;
  printf("%d %d %d %d %d %d\n", to_int(from_int(two)), to_int(from_int(256)),
         to_int(from_int(0)), to_int(from_long(1L << 40)),
         (int)from_uint(0x100u + two), to_int(from_enum(Wide::A)));
  printf("%d %d %d %d %d %d\n", to_bool(from_bool(true)),
         to_bool(from_bool(false)), to_int(from_enum(Wide::B)),
         to_int(from_double(0.0)), promote(UY), implicit(UY));
  unsigned char byte = static_cast<unsigned char>(from_int(-two));
  long wide = static_cast<long>(from_int(two));
  printf("%d %d %d %d\n", byte, (int)static_cast<P>(argc - 1), (int)wide,
         to_double(P::YES) == 1.0);
  int cond = 0;
  if (implicit(from_uint(4))) cond = 1;
  printf("%d %d %d\n", cond, !from_uint(two) ? 1 : 0,
         static_cast<int>(from_int(two)) + 2);
}
