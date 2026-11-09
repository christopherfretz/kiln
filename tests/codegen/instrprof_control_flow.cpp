// Source-based coverage counters (WP-941): under -fprofile-instr-generate
// every function gets a `__profc_` counter array and `__profd_` data
// record, and `llvm.instrprof.increment` calls at clang CodeGenPGO's
// region points: the function entry, each `if`/loop body, `case`/`default`,
// the statement after a `break`/`continue`/`return`/`goto` region, label,
// `?:` arm and the right operand of `&&`/`||`. The hash is clang's (with
// the 60-bit mask of clang 20+ under oracle 22). The lowering pass is
// disabled so the frontend's intrinsic calls are compared.
//@ args: -fprofile-instr-generate -Xclang -disable-llvm-passes
int ext(int);

int branches(int x, int y) {
  int r = 0;
  if (x > 0 && y > 0)
    r = 1;
  else if (x < 0 || y < 0)
    r = 2;
  r += x ? y : -y;
  return r;
}

int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    if (i == 3)
      continue;
    if (i == 7)
      break;
    s += i;
  }
  while (s > 100)
    s /= 2;
  do
    --s;
  while (s > 50);
  int a[] = {1, 2, 3};
  for (int v : a)
    s += v;
  return s;
}

int switches(int k) {
  switch (k) {
  case 0:
    return 10;
  case 1:
  case 2:
    k += 2;
    [[fallthrough]];
  case 5 ... 9:
    k *= 3;
    break;
  default:
    k = ext(k);
  }
  return k;
}

int jumps(int n) {
  int i = 0;
again:
  if (++i < n)
    goto again;
  try {
    ext(i);
  } catch (int e) {
    return e;
  } catch (...) {
    return -1;
  }
  return i;
}
