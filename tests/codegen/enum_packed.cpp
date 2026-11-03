// GNU extension: a `packed` enumeration without a fixed underlying type
// takes the narrowest of `unsigned char` / `unsigned short` (`signed
// char` / `short` with a negative value) that fits, before `int`, whether
// the attribute follows `enum` or the `}` (clang `ParseEnumBody`,
// `computeBestEnumTypes`); also a member enumeration of a template.
// (WP-432)

enum E { A } __attribute__((packed));
enum F { B = 300 } __attribute__((packed));
enum __attribute__((packed)) G { C };
enum N { M = -1 } __attribute__((packed));
enum N2 { M2 = -200, M3 = 100 } __attribute__((packed));
enum U3 { U3a = 70000 } __attribute__((packed));
enum class H { D } __attribute__((packed));
template <class T> struct TT { enum In { X = sizeof(T) } __attribute__((packed)); };
struct WithE { char c; E e; F f; };

E ge; F gf; G gg; N gn; N2 gn2; U3 gu3; H gh; TT<int>::In gin;
WithE gw = {1, A, B};
E ret(E x) { return x; }
F fv(F x) { return x; }
N2 neg(N2 x) { return x; }
int promote(E x) { return x + 1; }
int in(TT<int>::In x) { return x; }
