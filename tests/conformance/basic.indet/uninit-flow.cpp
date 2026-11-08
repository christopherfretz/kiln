// [basic.indet]/2, [dcl.init.general]/12: flow-based -Wuninitialized,
// -Wsometimes-uninitialized and -Wconditional-uninitialized for locals (clang's
// UninitializedValues analysis): straight-line code, if/else, loops, switch
// case and default, ?:, && and ||, uses after a call, self-initialization.
//@ expect: compile-ok
//@ args: -Wuninitialized -Wconditional-uninitialized
//@ strict-diags
//@ diag: 81:10 warning "variable 'x' is uninitialized when used here"
//@ diag: 80:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 86:3 warning "variable 'c' is uninitialized when used here"
//@ diag: 85:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 92:3 warning "variable 'c' is uninitialized when used here"
//@ diag: 91:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 98:7 warning "variable 'x' is used uninitialized whenever 'if' condition is false"
//@ diag: 100:10 note "uninitialized use occurs here"
//@ diag: 98:3 note "remove the 'if' if its condition is always true"
//@ diag: 97:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 105:7 warning "variable 'x' is used uninitialized whenever 'if' condition is false"
//@ diag: 109:10 note "uninitialized use occurs here"
//@ diag: 105:3 note "remove the 'if' if its condition is always true"
//@ diag: 104:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 125:12 warning "variable 's' is used uninitialized whenever 'if' condition is false"
//@ diag: 127:10 note "uninitialized use occurs here"
//@ diag: 125:8 note "remove the 'if' if its condition is always true"
//@ diag: 122:8 note "initialize the variable 's' to silence this warning"
//@ diag: 136:10 warning "variable 'q' may be uninitialized when used here"
//@ diag: 131:8 note "initialize the variable 'q' to silence this warning"
//@ diag: 143:10 warning "variable 's' may be uninitialized when used here"
//@ diag: 140:8 note "initialize the variable 's' to silence this warning"
//@ diag: 150:10 warning "variable 's' may be uninitialized when used here"
//@ diag: 147:8 note "initialize the variable 's' to silence this warning"
//@ diag: 160:10 warning "variable 's' may be uninitialized when used here"
//@ diag: 154:8 note "initialize the variable 's' to silence this warning"
//@ diag: 177:8 warning "variable 'r' is used uninitialized whenever switch case is taken"
//@ diag: 182:10 note "uninitialized use occurs here"
//@ diag: 172:8 note "initialize the variable 'r' to silence this warning"
//@ diag: 191:3 warning "variable 'r' is used uninitialized whenever switch default is taken"
//@ diag: 194:10 note "uninitialized use occurs here"
//@ diag: 186:8 note "initialize the variable 'r' to silence this warning"
//@ diag: 199:3 warning "variable 's' is used uninitialized whenever '?:' condition is false"
//@ diag: 200:10 note "uninitialized use occurs here"
//@ diag: 199:3 note "remove the '?:' if its condition is always true"
//@ diag: 198:8 note "initialize the variable 's' to silence this warning"
//@ diag: 205:7 warning "variable 's' is used uninitialized whenever '&&' condition is false"
//@ diag: 207:10 note "uninitialized use occurs here"
//@ diag: 205:7 note "remove the '&&' if its condition is always true"
//@ diag: 204:8 note "initialize the variable 's' to silence this warning"
//@ diag: 212:7 warning "variable 's' is used uninitialized whenever '||' condition is true"
//@ diag: 214:10 note "uninitialized use occurs here"
//@ diag: 212:7 note "remove the '||' if its condition is always false"
//@ diag: 211:8 note "initialize the variable 's' to silence this warning"
//@ diag: 219:5 warning "variable 'c' is uninitialized when used here"
//@ diag: 218:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 226:5 warning "variable 'acc' is uninitialized when used here"
//@ diag: 224:10 note "initialize the variable 'acc' to silence this warning"
//@ diag: 235:10 warning "variable 'x' may be uninitialized when used here"
//@ diag: 231:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 240:7 warning "variable 'p' is used uninitialized whenever 'if' condition is false"
//@ diag: 242:11 note "uninitialized use occurs here"
//@ diag: 240:3 note "remove the 'if' if its condition is always true"
//@ diag: 239:9 note "initialize the variable 'p' to silence this warning"
//@ diag: 247:7 warning "variable 'd' is used uninitialized whenever 'if' condition is false"
//@ diag: 249:10 note "uninitialized use occurs here"
//@ diag: 247:3 note "remove the 'if' if its condition is always true"
//@ diag: 246:11 note "initialize the variable 'd' to silence this warning"
//@ diag: 254:7 warning "variable 'v' is used uninitialized whenever 'if' condition is false"
//@ diag: 256:10 note "uninitialized use occurs here"
//@ diag: 254:3 note "remove the 'if' if its condition is always true"
//@ diag: 253:9 note "initialize the variable 'v' to silence this warning"
//@ diag: 264:7 warning "variable 'm' is used uninitialized whenever 'if' condition is false"
//@ diag: 267:13 note "uninitialized use occurs here"
//@ diag: 264:3 note "remove the 'if' if its condition is always true"
//@ diag: 263:12 note "initialize the variable 'm' to silence this warning"
//@ diag: 271:11 warning "variable 's' is uninitialized when used within its own initialization"
//@ diag: 276:11 warning "variable 's' is uninitialized when used within its own initialization"
int g(int);
void sink(int);

int straight() {
  int x;
  return x;
}

int compound() {
  int c;
  c += 1;
  return c;
}

int increment() {
  int c;
  c++;
  return c;
}

int if_then(bool b) {
  int x;
  if (b)
    x = 1;
  return x;
}

int if_else(bool b) {
  int x;
  if (b)
    x = 1;
  else
    sink(0);
  return x;
}

int both_arms(bool b) {
  int x;
  if (b)
    x = 1;
  else
    x = 2;
  return x;
}

int else_if(bool a, bool b) {
  int s;
  if (a)
    s = 1;
  else if (b)
    s = 2;
  return s;
}

int while_loop(bool b) {
  int q;
  while (b) {
    q = 1;
    b = false;
  }
  return q;
}

int for_loop(int n) {
  int s;
  for (int i = 0; i < n; ++i)
    s = i;
  return s;
}

int range_for(int (&a)[3]) {
  int s;
  for (int e : a)
    s = e;
  return s;
}

int do_while(bool b) {
  int s;
  do {
    if (b)
      break;
    s = 1;
  } while (b);
  return s;
}

int do_while_ok(bool b) {
  int s;
  do {
    s = 1;
  } while (b);
  return s;
}

int switch_case(int k) {
  int r;
  switch (k) {
  case 1:
    r = 1;
    break;
  case 2:
    break;
  default:
    r = 3;
  }
  return r;
}

int switch_default(int k) {
  int r;
  switch (k) {
  case 1:
    r = 1;
    break;
  default:
    break;
  }
  return r;
}

int ternary(bool a) {
  int s;
  a ? (s = 1) : 0;
  return s;
}

int logical_and(bool a) {
  int s;
  if (a && (s = 1)) {
  }
  return s;
}

int logical_or(bool a) {
  int s;
  if (a || (s = 1)) {
  }
  return s;
}

int after_call() {
  int c;
  g(c);
  return 0;
}

int in_loop(int n) {
  int acc;
  for (int i = 0; i < n; ++i)
    acc += i;
  return acc;
}

int conditional(bool b) {
  int x;
  for (int i = 0; i < 2; ++i)
    if (b)
      x = i;
  return x;
}

int pointer_fixit(bool b) {
  int *p;
  if (b)
    p = nullptr;
  return *p;
}

double double_fixit(bool b) {
  double d;
  if (b)
    d = 1.0;
  return d;
}

bool bool_fixit(bool b) {
  bool v;
  if (b)
    v = true;
  return v;
}

struct P {
  int x;
};
int member_pointer_fixit(bool b) {
  int P::*m;
  if (b)
    m = &P::x;
  P p{1};
  return p.*m;
}

int self_init() {
  int s = s + 1;
  return s;
}

int self_init_bare() {
  int s = s;
  return s;
}
