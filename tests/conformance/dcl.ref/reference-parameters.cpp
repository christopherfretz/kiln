// [dcl.ref] + [dcl.init.ref]: a reference is an alias of the object it is
// bound to; modifications through it are visible in the referee.
//@ expect: run
//@ stdout: 2 1 | 11 | 42
extern "C" int printf(const char*, ...);

void swap(int& a, int& b) {
  int t = a;
  a = b;
  b = t;
}
int& pick(int& a, int& b, bool first) { return first ? a : b; }

struct Box {
  int v;
};
void set(Box& b, int v) { b.v = v; }

int main() {
  int x = 1, y = 2;
  swap(x, y);
  printf("%d %d | ", x, y);
  pick(x, y, false) += 10;
  int& r = y;
  printf("%d | ", r);
  Box box = {0};
  set(box, 42);
  printf("%d\n", box.v);
}
