//@ mode: obj
//@ args: -O2
//@ budget-time: 10500
// (budget: 1.5x the recorded 7.0 s `-c`; the inline form takes minutes)
// One registration function performing 4,000 bindings, each a
// full-expression that builds a descriptor through an inlined member
// template and destroys an argument temporary with a non-trivial
// destructor, so every binding has an invoke and a landing pad (WP-641).
// After inlining, the function is one long chain of mergeable blocks
// behind an entry block holding every descriptor's alloca, and
// JumpThreading's block merging is quadratic in it: `opt -O2` took 18 s
// at 2,000 bindings and 91 to 116 s here as one function. Kiln now
// outlines runs of statements of a function this wide into internal
// noinline helpers (spec/landed/codegen/WP-641.md): 6.2 s.
namespace r {
struct arg_rec { const char *name; const void *def; unsigned flags; unsigned short pos; bool convert; };
struct rec {
  const char *name = nullptr; const char *doc = nullptr; void *impl = nullptr;
  unsigned nargs = 0; unsigned flags = 0; unsigned policy = 0; bool is_method = false; bool is_ctor = false;
};
void init(rec *, arg_rec *) noexcept;
inline const char *pick(bool b, const char *s) { if (b) return s; return nullptr; }
struct obj { long rc; };
void release(obj *) noexcept;
struct arg {
  const char *name; obj *def = nullptr;
  explicit arg(const char *n) : name(n) {}
  arg(arg &&o) : name(o.name), def(o.def) { o.def = nullptr; }
  ~arg() { if (def && --def->rc == 0) release(def); }
};
inline unsigned policy_of(unsigned n) { if (n > 2) return 3; if (n > 0) return 1; return 0; }
struct scope {
  template <class F> scope &def(const char *n, F &&f, unsigned nargs, bool doc, arg an) {
    rec R;
    R.name = n;
    if (const char *d = pick(doc, n)) { R.doc = d; R.flags |= 8; R.is_method = true; R.policy = 7; }
    auto *fp = +f; R.impl = reinterpret_cast<void *>(fp);
    R.nargs = nargs; R.flags |= 1; R.policy |= policy_of(nargs);
    if (R.nargs > 1) R.is_method = true;
    arg_rec A{};
    if (const char *a = pick(nargs != 0, an.name)) { A.name = a; A.flags = 2; A.pos = 1; A.convert = nargs < 2; }
    init(&R, &A);
    return *this;
  }
};
} // namespace r

#define STR_(x) #x
#define STR(x) STR_(x)
#define BIND(i) \
  m.def("f" STR(i), [](void **a) -> void * { return a[(i) % 5]; }, (i) % 4, (i) % 3 != 0, r::arg("x"));
#define B10(i) BIND(i##0) BIND(i##1) BIND(i##2) BIND(i##3) BIND(i##4) BIND(i##5) BIND(i##6) BIND(i##7) BIND(i##8) BIND(i##9)
#define B100(i) B10(i##0) B10(i##1) B10(i##2) B10(i##3) B10(i##4) B10(i##5) B10(i##6) B10(i##7) B10(i##8) B10(i##9)
#define B1000(i) B100(i##0) B100(i##1) B100(i##2) B100(i##3) B100(i##4) B100(i##5) B100(i##6) B100(i##7) B100(i##8) B100(i##9)

void register_all(r::scope &m) {
  B1000(1) B1000(2) B1000(3) B1000(4)
}
