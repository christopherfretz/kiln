// [lex.phases]/1.2, /1.3, [cpp.cond]: a directive's line is its logical
// line, past line splices and past a block comment that continues on later
// lines; in a live group and in a skipped one, each directive's range ends
// there and its group starts there (spec/pp.md "Preprocessing record",
// conditionals).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: logical-lines.record
#if 0 /* a comment
         over two lines */
int skipped;
#if 1 /* nested,
         dead */
#el\
if 2
#endif
#el\
if 1 + \
  1
int live;
#endif
#if 1 /* again
         live */
int kept;
#el\
if 3
#endif
