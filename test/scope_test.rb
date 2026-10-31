# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  class ScopeTest < Test
    def test_nesting_tightens_and_restores_the_ambient_deadline
      DeadlineMachines.with_deadline(5) do |outer|
        DeadlineMachines.with_deadline(1) do |inner|
          assert_same inner, DeadlineMachines.current
          assert_in_delta 1.0, DeadlineMachines.remaining
        end

        assert_same outer, DeadlineMachines.current
        assert_in_delta 5.0, DeadlineMachines.remaining
      end

      assert_nil DeadlineMachines.current
    end

    def test_a_nested_scope_without_a_limit_inherits_the_ambient_budget
      DeadlineMachines.with_deadline(5) do |outer|
        DeadlineMachines.with_deadline do |inner|
          refute_same outer, inner
          assert_in_delta 5.0, inner.remaining
        end
      end
    end

    def test_a_nested_scope_without_a_limit_still_sees_cancellation
      DeadlineMachines.with_deadline(5) do |outer|
        DeadlineMachines.with_deadline do |inner|
          outer.cancel('client hung up')

          assert_raises(Cancelled) { inner.check! }
        end
      end
    end

    def test_a_nested_scope_cannot_widen_the_budget
      DeadlineMachines.with_deadline(1) do
        DeadlineMachines.with_deadline(60) do
          assert_in_delta 1.0, DeadlineMachines.remaining
        end
      end
    end

    def test_the_ambient_deadline_is_restored_when_the_body_raises
      assert_raises(RuntimeError) do
        DeadlineMachines.with_deadline(5) { raise 'boom' }
      end

      assert_nil DeadlineMachines.current
    end

    def test_checks_outside_a_scope_are_free
      assert_nil DeadlineMachines.check!
      assert_equal Float::INFINITY, DeadlineMachines.remaining
      assert_in_delta 7.0, DeadlineMachines.clamp_sleep(7)
    end

    def test_a_child_fiber_inherits_the_ambient_deadline
      DeadlineMachines.with_deadline(5) do |outer|
        assert_same outer, Fiber.new { DeadlineMachines.current }.resume
      end
    end

    def test_a_fiber_cannot_leak_its_deadline_back_to_the_parent
      DeadlineMachines.with_deadline(5) do |outer|
        Fiber.new { DeadlineMachines.bind(Deadline.after(1, clock: @clock)) { nil } }.resume

        assert_same outer, DeadlineMachines.current
      end
    end

    def test_cancelling_the_ambient_deadline_stops_the_next_checkpoint
      DeadlineMachines.with_deadline(5) do |deadline|
        deadline.cancel('shutting down')

        error = assert_raises(Cancelled) { DeadlineMachines.check! }
        assert_equal 'shutting down', error.reason
      end
    end

    def test_an_exhausted_ambient_deadline_stops_the_next_checkpoint
      DeadlineMachines.with_deadline(5) do
        @clock.advance(6)

        assert_raises(Exceeded) { DeadlineMachines.check! }
      end
    end

    def test_deadline_duck_type_accepts_a_deadline_and_rejects_a_number
      assert DeadlineMachines.deadline?(Deadline.never(clock: @clock))
      refute DeadlineMachines.deadline?(5)
    end
  end
end
