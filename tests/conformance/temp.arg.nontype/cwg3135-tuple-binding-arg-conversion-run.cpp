//@ expect: run
//@ skip(clang): clang 22 keeps the reference model (the binding names a lifetime-extended temporary) and rejects the argument
//@ skip(gcc): GCC 16.1 mangles the argument without the qualification conversion (_Z2tcIL_Z1bEEiv), as it does for any variable
//@ stdout: 2 2
// [temp.arg.nontype]/6, [dcl.struct.bind]/7 (CWG 3135): the variable a
// non-constexpr tuple-like binding names (type int) bound to a `const
// int&` parameter. Kiln mangles it as it mangles any variable of another
// type than the parameter's referent, as clang does: the converted
// expression `XcvRKiL_Z1bEE`. GCC 16.1 has `L_Z1bE` for both (the asm
// label below links only if the instantiation has kiln's name).
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
} // namespace std
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 1; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
extern "C" int printf(const char *, ...);
template <const int& R> int tc() { return R; }
auto [a, b] = X{};
template int tc<b>();
int probe() __asm__("_Z2tcIXcvRKiL_Z1bEEEiv");
int main() {
  int x = tc<b>();
  int y = probe();
  printf("%d %d\n", x, y);
}
