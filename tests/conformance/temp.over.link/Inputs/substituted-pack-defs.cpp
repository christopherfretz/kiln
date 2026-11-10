// The definitions for ../substituted-pack-two-units-run.cpp, each under
// the name clang 19 and 22 give the specialization there.
template <class... T> struct tup {};
template <class... T> struct tl {};
struct B;
static int calls;
tup<int, int> cf(int &, int &&, double &&) __asm__(
    "_ZN4connI3idxIJLm0ELm1EEEE1fIiJidEEE3tupIJDpN2rsIT0_1RIT_X_SUBSTPACK_EEE4typeEEERS8_DpOS6_");
void cg(int &, int &&, double &&) noexcept __asm__(
    "_ZN4connI3idxIJLm0ELm1EEEE1gIiJidEEEvRT_DpOT0_");
tup<tl<int, int>, tl<double, int>> ch(int &, int, double) __asm__(
    "_ZN4connI3idxIJLm0ELm1EEEE1hIiJidEEE3tupIJDp2tlIJ1RIT0_X_SUBSTPACK_EES6_IT_X_SUBSTPACK_EEEEEERS9_DpS7_");
tup<tl<int, int>, tl<char, double>> tf(int, double) __asm__(
    "_ZN2tyIJicEE1fIJidEEE3tupIJDp2tlIJ_SUBSTPACK_T_EEEEDpS5_");
B *nb(int) __asm__("_Z2nbIiEDTcmfp_nw_1BpiLi1ELi2EEET_");
tup<int, int> cf(int &s, int &&, double &&) {
  ++s;
  ++calls;
  return {};
}
void cg(int &s, int &&, double &&) noexcept {
  ++s;
  ++calls;
}
tup<tl<int, int>, tl<double, int>> ch(int &, int, double) {
  ++calls;
  return {};
}
tup<tl<int, int>, tl<char, double>> tf(int, double) {
  ++calls;
  return {};
}
B *nb(int) {
  ++calls;
  return nullptr;
}
int count() { return calls - 1; }
