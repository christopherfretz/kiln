// [expr.new]: new-expressions in the arms of ?: and on the right of && are
// evaluated only when their arm is.
//@ expect: run
//@ stdout: 1 2 | yes
extern "C" int printf(const char*, ...);

int main(int argc, char**) {
  int* p = argc > 0 ? new int(1) : new int(2);
  int* q = argc > 5 ? new int(1) : new int(2);
  printf("%d %d | ", *p, *q);
  int* r = nullptr;
  if (argc > 0 && (r = new int(3)) != nullptr)
    printf("yes\n");
  delete r;
  delete q;
  delete p;
}
