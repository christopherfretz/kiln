// [expr.const]: the values of the register-local escape cases
// (Inputs/cases.h, spec/oven.md "Register locals"), checked with every
// compiler; cases-oven.cpp checks which locals kiln keeps in registers.
#include "Inputs/cases.h"
static_assert(q_addr(1) == 2);
static_assert(m_addr(1) == 2);
static_assert(q_ref(2) == 4);
static_assert(m_ref(1) == 2);
static_assert(m_ref_param(1) == 2);
static_assert(q_capture_copy(1) == 2);
static_assert(m_capture_ref(1) == 2);
static_assert(m_capture_default_ref(1) == 2);
static_assert(m_addressof(1) == 2);
static_assert(m_builtin_addressof(1) == 2);
static_assert(q_move(1, 2) == 21);
static_assert(q_forward(1) == 4);
static_assert(m_move_bound(1) == 2);
static_assert(q_unevaluated(1) == 9);
static_assert(m_unevaluated_and_ref(1) == 2);
static_assert(q_const_read(2) == 4);
static_assert(m_const_cast(1) == 2);
static_assert(q_assigned_first(3) == 6);
static_assert(q_assigned_both_branches(true) == 1);
static_assert(q_assigned_both_branches(false) == 2);
static_assert(q_assigned_each_iteration(4) == 6);
static_assert(m_assigned_one_branch(true) == 1);
static_assert(m_assigned_one_branch(false) == 2);
static_assert(m_case_in_scope(0) == 1);
static_assert(q_case_out_of_scope(0) == 1);
static_assert(q_reassigned(2) == 4);
static_assert(m_assigned_in_loop(5) == 5);
