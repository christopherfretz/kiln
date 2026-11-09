// The other unit of ../lambda-in-signature-member-template-return-two-units-run.cpp:
// it calls the members and closures of `S<int>` and `C<int>` under the
// names clang 19 and 22 give them (asm labels), so the program links only
// if kiln defines the same names. A call operator takes the closure as
// its first argument (`this`).
#include <cstdio>
struct Closure {};
Closure mret_int(int) __asm__("_ZN1SIiE4mretIiEENS0_UlvE_ET_");
int mret_call(const Closure *) __asm__("_ZZN1SIiE4mretET_ENKUlvE_clEv");
Closure nf_int(int) __asm__("_ZN1SIiE1NIiE1fIiEENS2_UlvE_ET_");
int nf_call(const Closure *) __asm__("_ZZN1SIiE1NIiE1fET_ENKUlvE_clEv");
extern const Closure vt_int __asm__("_ZN1SIiE2vtIiEE");
int vt_call(const Closure *) __asm__("_ZNK1SIiEUlvE0_clEv");
Closure mgen_int(int) __asm__("_ZN1SIiE4mgenIiEENS0_UlTL0__E_ET_");
int mgen_call(const Closure *, int) __asm__("_ZZN1SIiE4mgenET_ENKUlS1_E_clIiEEDaS1_");
void *mcap_int(int) __asm__("_ZN1CIiE4mcapIiEEPNS0_UlvE_ET_");
void *mref_int(int) __asm__("_ZN1CIiE4mrefIiEEPNS0_UlvE0_ET_");
void other_members() {
  Closure r = mret_int(1), n = nf_int(1), g = mgen_int(1);
  std::printf("%d %d %d %d %d\n", mret_call(&r), nf_call(&n), vt_call(&vt_int), mgen_call(&g, 2),
              !mcap_int(1) + !mref_int(1) + 1);
}
