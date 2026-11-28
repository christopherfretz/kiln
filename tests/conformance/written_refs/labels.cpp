// WP-3688: label uses (`goto l`, `&&l`, `asm goto` label operands) are
// recorded in the written-reference table with role `label`, including
// forward uses, nested blocks, lambdas, `__label__` local labels, and
// function templates; computed `goto *p` records no label ref for `*p`.
//@ written-refs: labels.refs
//@ args: -fkiln-written-refs -w
void g() { f: ; goto f; void *p = &&f; (void)p; }
void h(int x) { asm goto ("" : : "r"(x) : : out); out: ; }
void k() { __label__ m; goto m; m: ; }
void fwd() { goto later; later: ; }
void nested_and_lambda() {
  top: ;
  { goto top; }
  { __label__ top; goto top; top: ; }
  auto fn = []() { top: ; goto top; };
  (void)fn;
}
template <class T> void tmpl(T x) {
  l: ;
  void *p = &&l;
  (void)p;
  (void)x;
}
void use_tmpl() {
  tmpl(1);
  tmpl(2.0);
}
void computed(int i) {
  a: ;
  b: ;
  void *t[2] = {&&a, &&b};
  void *p = t[i];
  goto *p;
}
void multi_asm(int x) { asm goto ("" : : "r"(x) : : l1, l2); l1: ; l2: ; }
