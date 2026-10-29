//@ expect: ast
// GNU statement expressions: the value is the last expression statement's
// (decayed, lvalue-to-rvalue converted), looking through labels and past
// trailing null statements; otherwise void.
int arr[3];
const int ci = 1;
int g;
void v();
int fn();
int &rf();
int f() {
  auto a = ({ arr; });
  auto b = ({ ci; });
  auto c = ({ fn; });
  ({ v(); });
  auto d = ({ rf(); });
  auto h = ({ ci + 1; ; });
  int x = ({ int y = 2; y + 1; });
  ({ ; });
  ({ int q = 1; (void)q; });
  ({ g = 1; });
  int z = ({ ({ g; }) + 1; });
  return ({ x; }) + z;
}
