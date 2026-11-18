// [basic.link]/11, [intro.object]/9: an exported const variable is one
// object, so an importer sees the module unit's address although it
// copies the value `available_externally` (WP-1653).
//@ expect: run
//@ std: c++20
//@ module: const_vars=Inputs/const-vars.cppm
//@ stdout: 1 1 1 1 sum 23
import const_vars;
extern "C" int printf(const char *, ...);

int main() {
  const void *a[] = {&c1, &c3, &arr[1], &vt<long>};
  for (int i = 0; i < 4; ++i)
    printf("%d ", a[i] == module_address(i));
  printf("sum %d\n", c1 + c3 + arr[1] + int(vt<long>));
}
