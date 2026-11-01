// [stmt.goto], [stmt.jump]/2, [stmt.dcl]/2: a goto out of nested scopes
// (forward and backward, out of loops and try blocks) runs the
// destructors of every scope it exits, once, through clang's cleanup
// switch; a local with a trivial default constructor and a non-trivial
// destructor still gets its normal and EH cleanups.
extern "C" int printf(const char*, ...);
struct D {
  int id;
  ~D() { printf("~D%d ", id); }
};
struct T { ~T(); };
T::~T() { printf("~T "); }
void may_throw(bool b) { if (b) throw 7; }

int fwd(bool b) {
  T t0;
  if (b) goto out;
  printf("body ");
out:
  return 3;
}

void nested(int n) {
  D a{1};
  {
    D b{2};
    {
      D c{3};
      if (n == 0) goto done;
      if (n == 1) goto mid;
    }
  mid:
    printf("mid ");
  }
done:
  printf("done\n");
}

void backward() {
  int i = 0;
again:
  {
    D x{10 + i};
    ++i;
    if (i < 3) goto again;
  }
  printf("end\n");
}

void loop_out() {
  for (int i = 0;; ++i) {
    D l{20 + i};
    for (int j = 0;; ++j) {
      D m{30 + j};
      if (j == 1 && i == 1) goto out;
      if (j == 1) break;
    }
  }
out:
  printf("out\n");
}

void eh(bool b) {
  try {
    D a{40};
    if (!b) goto skip;
    may_throw(b);
  skip:
    printf("noexc ");
  } catch (int) {
    printf("caught ");
  }
  printf("\n");
}

void try_out() {
  try {
    D a{50};
    goto leave;
  } catch (...) {}
leave:
  printf("left\n");
}

int asmgoto(int x) {
  D a{60};
  asm goto("" : : "r"(x) : : lab);
  printf("fall ");
  return 0;
lab:
  return 1;
}

