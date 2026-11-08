//@ expect: run
//@ strict-diags
//@ stdout: 1 2 4 8 1 2 1
//@ stdout: 0 1 -1
// GCC mode on an enumeration (clang `AddModeAttr` on an `EnumDecl`): an
// integer mode sets the underlying type, from `int` when it is not fixed
// (so signed, whatever the enumerators) and from the fixed type otherwise
// (keeping its signedness). Promotion follows the new underlying type.
// kiln ignored it, giving every such enumeration the size of `int`.
#include <cstdio>

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

enum F { Y } __attribute__((mode(QI)));
enum G { Z = 200 } __attribute__((mode(HI)));
enum __attribute__((mode(SI))) H { W = -1 };
enum U { A = 1u << 31 } __attribute__((mode(DI)));
enum Fx : int { Zx } __attribute__((mode(QI)));
enum class C : unsigned { Q = 1 } __attribute__((mode(HI)));
enum P { R } __attribute__((mode(QI), packed));

static_assert(same<__underlying_type(F), signed char>::value);
static_assert(same<__underlying_type(G), short>::value);
static_assert(same<__underlying_type(H), int>::value);
static_assert(same<__underlying_type(U), long>::value);
static_assert(same<__underlying_type(Fx), signed char>::value);
static_assert(same<__underlying_type(C), unsigned short>::value);
static_assert(same<decltype(+Y), int>::value);
static_assert(same<decltype(+Zx), int>::value);
static_assert(alignof(G) == 2);

int main() {
  std::printf("%zu %zu %zu %zu %zu %zu %zu\n", sizeof(F), sizeof(G), sizeof(H), sizeof(U),
              sizeof(Fx), sizeof(C), sizeof(P));
  std::printf("%d %d %d\n", (int)Y, (int)C::Q, (int)W);
}
