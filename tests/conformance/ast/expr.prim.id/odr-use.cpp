//@ expect: ast
constexpr int limit = 10;
int g;
int use(int v) {
  const int local = 3;
  int arr[limit];
  arr[0] = local;
  return v < limit ? arr[0] + g : local;
}
