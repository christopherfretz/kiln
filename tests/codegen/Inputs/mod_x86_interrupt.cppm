export module mod_x86_interrupt;
export struct frame {
  unsigned long ip, cs, flags, sp, ss;
};
export inline __attribute__((interrupt)) void isr(frame *) {}
export inline __attribute__((interrupt)) void isr_code(frame *, unsigned long) {}
export inline __attribute__((interrupt)) void isr_unnamed(frame *) {}
