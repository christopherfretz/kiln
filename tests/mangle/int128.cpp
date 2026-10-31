// __int128 / unsigned __int128 mangle as `n` / `o`, also through the
// implicit __int128_t / __uint128_t typedefs.
void f(__int128_t, __uint128_t) {}
void g(__int128 *, const unsigned __int128 &) {}
template <class T> void h(T) {}
void use() {
  h<__int128_t>(0);
  h<__uint128_t *>(nullptr);
}
