// [stmt.goto], [stmt.jump]/2, [stmt.dcl]/2, [except.ctor]: jumps out of
// scopes destroy the variables of each scope they exit, in reverse
// order, exactly once (forward, backward, out of loops and try blocks);
// a default-initialized local with a trivial constructor and a
// non-trivial destructor is destroyed.
//@ expect: run
//@ stdout: ~T,3
//@ stdout: body,~T,3
//@ stdout: ~D3,~D2,done
//@ stdout: ~D1,~D3,mid,~D2,done
//@ stdout: ~D1,~D3,mid,~D2,done
//@ stdout: ~D1,~D10,~D11,~D12,end
//@ stdout: ~D30,~D31,~D20,~D30,~D31,~D21,out
//@ stdout: noexc,~D40,
//@ stdout: ~D40,caught,
//@ stdout: ~D50,left
//@ stdout: fall,~D60,0
extern "C" int printf(const char*, ...);
struct D {
  int id;
  ~D() { printf("~D%d,", id); }
};
struct T { ~T(); };
T::~T() { printf("~T,"); }
void may_throw(bool b) { if (b) throw 7; }

int fwd(bool b) {
  T t0;
  if (b) goto out;
  printf("body,");
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
    printf("mid,");
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
    printf("noexc,");
  } catch (int) {
    printf("caught,");
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
  printf("fall,");
  return 0;
lab:
  return 1;
}

int main() {
  printf("%d\n", fwd(true));
  printf("%d\n", fwd(false));
  nested(0); nested(1); nested(2);
  backward();
  loop_out();
  eh(false); eh(true);
  try_out();
  printf("%d\n", asmgoto(1));
}
