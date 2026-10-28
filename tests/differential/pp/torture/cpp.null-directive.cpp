//@ expect: pp
// The null directive, a lone # on a line (possibly with comments),
// produces nothing, including inside skipped and active groups.
#
# // comment
  #  /* c */
a;
#if 0
#
#nonsense directive in a skipped group
#endif
#if 1
#
b;
#endif
