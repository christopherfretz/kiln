// [cpp.cond]/6, /13: conditionals nested in skipped groups, two levels
// deep, with `#elif` and `#else` of their own: every directive of them is
// dead and links to its own opener; the `#else` of the outer conditional
// is taken (spec/pp.md "Preprocessing record", conditionals).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: nested-skipped.record
#ifdef UNDEFINED_MACRO
#  if 1
#    ifndef X
#    elif Y
#    endif
#  else
#  endif
#elif 0
#  ifdef Z
#  endif
#else
int taken;
#endif
