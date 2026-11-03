//@ expect: run
//@ oracle: gcc
int main() {
#ifdef KILN_FAKE_GCC
  return 1;
#else
  return 0;
#endif
}
