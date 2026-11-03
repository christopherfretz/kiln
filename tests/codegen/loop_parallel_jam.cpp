// `loop_parallel.cpp` under an unroll-and-jam loop (clang 22 lists the
// inner loop's after-jam properties in `followup_inner` itself; clang 19,
// and kiln under `-fclang-compat=19`, made a distinct self-referential
// node of them, WP-492). The parallel inner loop's before-jam and
// after-jam properties both carry its access group
// (`llvm.loop.parallel_accesses`).
void jam(int *a, int n) {
#pragma unroll_and_jam(2)
  for (int i = 0; i < n; ++i)
#pragma clang loop vectorize(assume_safety) unroll_count(2)
    for (int j = 0; j < n; ++j) a[j] += i;
}
