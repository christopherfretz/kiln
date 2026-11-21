// The module of image-x86-interrupt-run.cpp: inline x86-64 interrupt
// handlers (implicitly `used`).
export module imgxi;
export struct frame {
  unsigned long ip, cs, flags, sp, ss;
};
export inline __attribute__((interrupt)) void isr(frame *) {}
export inline __attribute__((interrupt)) void isr_code(frame *, unsigned long) {}
export inline __attribute__((interrupt)) void isr_unnamed(frame *) {}
export void *module_isr() { return (void *)&isr; }
