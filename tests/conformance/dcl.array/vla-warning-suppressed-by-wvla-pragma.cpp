// `-Wvla-cxx-extension` is in clang's `-Wvla` group (as in
// `-Wvla-extension`): `#pragma GCC diagnostic ignored "-Wvla"` silences
// a C++ variable length array. (folly's AsyncUDPSocket.cpp suppresses
// its VLAs this way.)
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
int f(int n) {
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wvla"
  char buf[n];
#pragma GCC diagnostic pop
  buf[0] = 1;
  return buf[0];
}

int main() { return f(2) - 1; }
