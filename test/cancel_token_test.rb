# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  # The runtime cancellation tree: what it delivers, and what it lets go of.
  class CancelTokenTest < Test
    def test_cancellation_flows_down_but_not_up
      root = CancelToken.root
      child = root.child
      grandchild = child.child
      sibling = root.child

      assert_equal :propagated, child.cancel

      assert_predicate child, :cancelled?
      assert_predicate grandchild, :cancelled?
      refute_predicate sibling, :cancelled?
      refute_predicate root, :cancelled?
    end

    def test_a_child_of_a_cancelled_token_is_born_cancelled
      root = CancelToken.root
      root.cancel

      assert_predicate root.child, :cancelled?
    end

    # A repeat, or a cancel on a token an ancestor already cancelled, is an
    # honoured repeat that changes nothing, its reason included.
    def test_a_repeat_is_already_cancelled_and_keeps_the_first_reason
      root = CancelToken.root
      child = root.child

      assert_equal :propagated, root.cancel('first')
      assert_equal :already_cancelled, root.cancel('second')
      assert_equal :already_cancelled, child.cancel('late')
      assert_equal :already_cancelled, root.child.cancel('later')
      assert_equal 'first', root.reason
      assert_equal 'first', child.reason
    end

    def test_reason_is_inherited_from_the_nearest_ancestor
      root = CancelToken.root
      grandchild = root.child.child
      root.cancel('client hung up')

      assert_equal 'client hung up', grandchild.reason
      assert_nil CancelToken.root.reason
    end

    def test_on_cancel_fires_once_and_fires_late_registrations_immediately
      token = CancelToken.root
      fired = []
      token.on_cancel { fired << :before }
      token.cancel
      token.cancel
      token.on_cancel { |seen| fired << (seen.equal?(token) ? :after : :wrong) }

      assert_equal %i[before after], fired
    end

    def test_new_is_private_and_inspect_names_the_state
      assert_raises(NoMethodError) { CancelToken.new(nil) }
      token = CancelToken.root
      assert_equal '#<DeadlineMachines::CancelToken live>', token.inspect
      token.cancel
      assert_equal '#<DeadlineMachines::CancelToken cancelled>', token.inspect
    end

    def test_block_form_unlinks_the_child_even_when_the_body_raises
      root = CancelToken.root
      fired = []
      assert_equal :value, root.child { |child| child.on_cancel { fired << :kept } && :value }
      assert_raises(RuntimeError) do
        root.child do |child|
          child.on_cancel { fired << :raised }
          raise 'boom'
        end
      end

      root.cancel

      assert_empty fired
    end

    def test_an_unlinked_child_still_reports_the_tree_state
      root = CancelToken.root
      escaped = nil
      root.child { |child| escaped = child }

      root.cancel

      assert_predicate escaped, :cancelled?
    end

    def test_one_failing_observer_does_not_silence_its_peers
      fired = []
      token = CancelToken.root
      token.on_cancel { raise 'observer boom' }
      token.on_cancel { fired << :second }

      assert_raises(RuntimeError) { token.cancel }

      assert_equal [:second], fired
      assert_predicate token, :cancelled?
    end

    def test_a_failing_child_observer_does_not_silence_a_sibling
      seen = []
      root = CancelToken.root
      root.child.on_cancel { raise 'boom' }
      root.child.on_cancel { seen << :sibling }

      assert_raises(RuntimeError) { root.cancel }

      assert_equal [:sibling], seen
    end

    # Tokens the caller dropped, with nothing left to deliver to them.
    DISCARDED = 20_000

    # Registrations raced against a cancellation of the same token.
    RACERS = 2_000

    # A weak map still lists whatever the collector has not reached, including
    # the local the loop just wrote, so these tests ask that nothing
    # accumulates rather than that the collector has already run.
    SETTLED = 16

    # Seconds a test waits for a racing thread before calling it wedged.
    RENDEZVOUS = 5

    def test_a_child_the_caller_dropped_is_not_kept
      root = CancelToken.root
      DISCARDED.times { root.child }
      3.times { GC.start }

      assert_settled root
    end

    def test_a_cancelled_child_is_let_go_of
      root = CancelToken.root
      DISCARDED.times do
        child = root.child
        child.on_cancel { nil }
        child.cancel
      end
      3.times { GC.start }

      assert_settled root
    end

    def test_a_cancellation_racing_registration_leaves_nothing_retained
      root = CancelToken.root
      RACERS.times do
        child = root.child
        registering = Thread.new { child.on_cancel { nil } }
        child.cancel

        assert registering.join(RENDEZVOUS), 'a registration never finished'
      end
      3.times { GC.start }

      assert_empty retained(root)
    end

    def test_a_block_that_observed_its_child_leaves_nothing_behind
      root = CancelToken.root
      DISCARDED.times { root.child { |child| child.on_cancel { nil } } }
      3.times { GC.start }

      assert_empty retained(root)
    end

    def test_a_detached_subtree_does_not_reconnect_through_a_descendant
      root = CancelToken.root
      escaped = nil
      root.child { |child| escaped = child }
      deep = escaped.child.child
      fired = false
      deep.on_cancel { fired = true }
      3.times { GC.start }

      assert_empty retained(root)

      escaped.cancel

      assert fired
    end

    def test_an_observer_registered_after_a_block_ended_is_the_callers_own
      root = CancelToken.root
      escaped = nil
      root.child { |child| escaped = child }
      escaped.on_cancel { nil }
      3.times { GC.start }

      assert_empty retained(root)

      fired = false
      escaped.on_cancel { fired = true }
      escaped.cancel

      assert fired
    end

    def test_a_link_that_still_owes_its_own_observer_is_kept
      root = CancelToken.root
      middle = root.child
      seen = []
      middle.on_cancel { seen << :middle }
      leaf = middle.child
      leaf.on_cancel { nil }
      leaf.cancel
      3.times { GC.start }

      assert_equal [middle], retained(root)

      root.cancel

      assert_equal [:middle], seen
    end

    def test_a_sibling_cancellation_does_not_strand_a_fresh_observer
      RACERS.times do
        root = CancelToken.root
        middle = root.child
        doomed = middle.child
        doomed.on_cancel { nil }
        fired = false
        killer = Thread.new { doomed.cancel }
        middle.child.on_cancel { fired = true }

        assert killer.join(RENDEZVOUS), 'a cancellation never finished'
        root.cancel

        assert fired
      end
    end

    def test_an_observer_outlives_every_link_above_it
      root = CancelToken.root
      seen = []
      ->(parent) { parent.child.child.on_cancel { seen << :deep } }.call(root)
      3.times { GC.start }

      root.cancel

      assert_equal [:deep], seen
    end

    def test_a_cancelled_chain_is_let_go_of_link_by_link
      root = CancelToken.root
      DISCARDED.times do
        leaf = root.child.child
        leaf.on_cancel { nil }
        leaf.cancel
      end
      3.times { GC.start }

      assert_settled root
    end

    def test_an_observer_outlives_the_child_reference_the_caller_dropped
      root = CancelToken.root
      seen = []
      root.child.on_cancel { seen << :fired }
      3.times { GC.start }

      root.cancel

      assert_equal [:fired], seen
    end

    private

    def retained(token)
      token.instance_variable_get(:@retained).keys
    end

    # Nothing is owed through +root+, and its weak children have not grown with
    # the number of tokens the test discarded.
    def assert_settled(root)
      assert_empty retained(root)
      assert_operator root.instance_variable_get(:@children).keys.size, :<, SETTLED
    end
  end
end
