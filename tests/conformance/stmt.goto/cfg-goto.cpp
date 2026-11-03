// [stmt.goto]/1, [stmt.label]/1, [stmt.break]/1, [stmt.cont]/1: jumps end
// their block with an edge to the target; a label starts a block; a jump
// out of a scope runs its destructors first (the control-flow graph,
// spec/sema.md §7.5.4).
//@ expect: cfg
void g();
int h(int);

struct D {
  ~D();
};

int forward(int x) {
  if (x)
    goto done;
  g();
done:
  return x;
}

void backward(int x) {
again:
  g();
  if (h(x))
    goto again;
}

void scoped(int x) {
  while (x) {
    D d;
    if (h(x))
      break;
    if (h(x + 1))
      continue;
    g();
  }
}

void nested(int x) {
  for (int i = 0; i < x; ++i)
    for (int j = 0; j < x; ++j) {
      if (j == i)
        continue;
      if (h(j))
        goto out;
    }
out:
  g();
}
