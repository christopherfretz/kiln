//@ expect: ast
namespace std { class type_info; }
struct S { int m; };
int x;
const int cx = 1;
int arr[3];
void fn();
auto &t1 = typeid(int);
auto &t2 = typeid(const int &);
auto &t3 = typeid(S);
auto &t4 = typeid(x);
auto &t5 = typeid(cx);
auto &t6 = typeid(x + 1);
auto &t7 = typeid(arr);
auto &t8 = typeid(fn);
auto &t9 = typeid((x));
auto &t10 = typeid(int);
bool same = &typeid(int) == &typeid(x);
void f() {
  typeid(S);
  (void)typeid(int);
}
