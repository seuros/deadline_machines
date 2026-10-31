# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  class DeadlineTest < Test
    def test_derive_keeps_the_tighter_bound
      parent = Deadline.after(5, clock: @clock)

      assert_in_delta 1.0, parent.derive(1).remaining
      assert_in_delta 5.0, parent.derive(9).remaining
      assert_in_delta 5.0, parent.derive.remaining
    end

    def test_derive_without_a_parent_limit_takes_the_child_limit
      assert_in_delta 2.0, Deadline.never(clock: @clock).derive(2).remaining
      assert_equal Float::INFINITY, Deadline.never(clock: @clock).derive.remaining
    end

    def test_remaining_floors_at_zero_once_expired
      deadline = Deadline.after(1, clock: @clock)
      @clock.advance(10)

      assert_in_delta 0.0, deadline.remaining
      assert_predicate deadline, :expired?
    end

    def test_a_never_deadline_does_not_expire
      refute_predicate Deadline.never(clock: @clock), :expired?
      refute_predicate Deadline.never(clock: @clock), :done?
    end

    def test_cancellation_flows_down_but_not_up
      root = Deadline.never(clock: @clock)
      child = root.derive
      grandchild = child.derive
      sibling = root.derive

      child.cancel

      assert_predicate child, :cancelled?
      assert_predicate grandchild, :cancelled?
      refute_predicate sibling, :cancelled?
      refute_predicate root, :cancelled?
    end

    def test_cancelling_a_parent_reaches_a_child_derived_later
      root = Deadline.never(clock: @clock)
      root.cancel

      assert_predicate root.derive, :cancelled?
    end

    def test_cancel_reason_is_inherited_from_the_nearest_ancestor
      root = Deadline.never(clock: @clock)
      grandchild = root.derive.derive
      root.cancel('client hung up')

      error = assert_raises(Cancelled) { grandchild.check! }

      assert_equal 'client hung up', error.reason
    end

    def test_on_cancel_fires_once_and_fires_late_registrations_immediately
      deadline = Deadline.never(clock: @clock)
      fired = []
      deadline.on_cancel { fired << :before }
      deadline.cancel
      deadline.cancel
      deadline.on_cancel { fired << :after }

      assert_equal %i[before after], fired
    end

    def test_cancellation_outranks_expiry_when_both_apply
      deadline = Deadline.after(1, clock: @clock)
      deadline.cancel
      @clock.advance(10)

      assert_raises(Cancelled) { deadline.check! }
    end

    def test_check_returns_self_while_there_is_budget
      deadline = Deadline.after(1, clock: @clock)

      assert_same deadline, deadline.check!
    end

    def test_clamp_sleep_never_overruns_the_budget
      deadline = Deadline.after(1, clock: @clock)

      assert_in_delta 0.3, deadline.clamp_sleep(0.3)
      @clock.advance(0.8)
      assert_in_delta 0.2, deadline.clamp_sleep(0.3)
      @clock.advance(1)
      assert_in_delta 0.0, deadline.clamp_sleep(0.3)
      assert_in_delta 0.3, Deadline.never(clock: @clock).clamp_sleep(0.3)
    end

    def test_block_form_unlinks_the_child_from_callback_dispatch
      root = Deadline.never(clock: @clock)
      fired = false
      root.derive(1) { |child| child.on_cancel { fired = true } }

      root.cancel

      refute fired
    end

    def test_block_form_unlinks_even_when_the_body_raises
      root = Deadline.never(clock: @clock)
      fired = false
      assert_raises(RuntimeError) do
        root.derive(1) do |child|
          child.on_cancel { fired = true }
          raise 'boom'
        end
      end

      root.cancel

      refute fired
    end

    def test_an_unlinked_child_still_reports_the_tree_state
      root = Deadline.never(clock: @clock)
      escaped = nil
      root.derive(1) { |child| escaped = child }

      root.cancel

      assert_predicate escaped, :cancelled?
    end
    def test_one_failing_observer_does_not_silence_its_peers
      fired = []
      deadline = Deadline.never(clock: @clock)
      deadline.on_cancel { raise 'observer boom' }
      deadline.on_cancel { fired << :second }

      assert_raises(RuntimeError) { deadline.cancel }

      assert_equal [:second], fired
    end

    def test_a_failing_child_observer_does_not_silence_a_sibling
      seen = []
      root = Deadline.never(clock: @clock)
      root.derive(5).on_cancel { raise 'boom' }
      root.derive(5).on_cancel { seen << :sibling }

      assert_raises(RuntimeError) { root.cancel }

      assert_equal [:sibling], seen
    end

  end
end
