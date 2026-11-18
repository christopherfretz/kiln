// Module of const-var-address-run.cpp.
export module const_vars;
export constexpr int c1 = 5;
export const int c3 = 7;
export constexpr int arr[3] = {1, 2, 3};
export template <class T> constexpr T vt = T(3);
export template <> constexpr long vt<long> = 9;
export const void *module_address(int i) {
  const void *a[] = {&c1, &c3, &arr[1], &vt<long>};
  return a[i];
}
