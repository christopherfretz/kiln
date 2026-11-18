// Module of const-ref-temporary-address-run.cpp.
export module const_refs;
export constexpr int &&d4 = 3;
int f() { return 4; }
export const int &d5 = f();
export const void *module_address(int i) {
  const void *a[] = {&d4, &d5};
  return a[i];
}
