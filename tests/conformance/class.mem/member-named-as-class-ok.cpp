// [class.mem.general]/22 valid neighbours (WP-1015): a scoped enumerator, a
// non-static data member when no constructor is user-declared, member
// functions, elaborated and friend references to the injected-class-name,
// and the class's name inside a nested class all keep compiling.
//@ std: c++20
//@ expect: compile-ok
struct E0 { enum class K { E0 }; };
struct F0 { int F0; };
struct G0 { struct G0 *next; friend struct G0; G0(); };
struct H0 { struct Inner { int H0; static int x; }; };
struct I0 { int f(); static int g; enum { A, B }; };
struct B { int J0; };
struct J0 : B { using B::J0; };
template<class T> struct K0 { int K0; };
K0<int> k0;
int main() { F0 f; f.F0 = 1; J0 j; j.J0 = 2; return E0::K::E0 == E0::K{} ? 0 : 1; }
