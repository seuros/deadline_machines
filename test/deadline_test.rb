# frozen_string_literal: true

require 'test_helper'

module DeadlineMachines
  # The value: native arithmetic through the core, and values that are valid
  # or do not exist.
  class DeadlineTest < Test
    MAX = 2**64 - 1
    # The core's Duration::MAX in nanoseconds.
    DURATION_MAX = (MAX * 1_000_000_000) + 999_999_999

    def test_derive_keeps_the_tighter_bound
      parent = Deadline.after(0, 5_000)

      assert_equal Deadline.at(1_000), parent.derive(0, 1_000)
      assert_equal parent, parent.derive(0, 9_000)
      assert_equal Deadline.at(9_000), Deadline.never.derive(0, 9_000)
    end

    def test_tighten_is_commutative_and_absorbs_never
      a = Deadline.after(0, 10)
      b = Deadline.after(0, 20)

      assert_equal a, a.tighten(b)
      assert_equal a, b.tighten(a)
      assert_equal a, a.tighten(Deadline.never)
      assert_equal Deadline.never, Deadline.never.tighten(Deadline.never)
    end

    def test_remaining_floors_at_zero_and_never_has_none
      deadline = Deadline.after(0, 100)

      assert_equal 60, deadline.remaining(40)
      assert_equal 0, deadline.remaining(400)
      assert deadline.expired?(100)
      refute deadline.expired?(99)
      assert_nil Deadline.never.remaining(DURATION_MAX)
      refute Deadline.never.expired?(DURATION_MAX)
    end

    def test_check_answers_a_refusal_symbol_about_that_instant_only
      deadline = Deadline.after(0, 100)

      assert_nil deadline.check(99)
      assert_equal :exceeded, deadline.check(100)
      assert_nil deadline.check(15)
      assert_equal %i[exceeded cancelled], REFUSALS
      assert_predicate REFUSALS, :frozen?
    end

    def test_extend_by_pushes_back_and_sums_saturate_at_the_end_of_time
      assert_equal Deadline.at(150), Deadline.after(0, 100).extend_by(50)
      assert_equal Deadline.never, Deadline.never.extend_by(50)
      assert_equal Deadline.at(DURATION_MAX), Deadline.after(DURATION_MAX, 10)
      assert_equal Deadline.at(DURATION_MAX), Deadline.at(DURATION_MAX).extend_by(1)
    end

    def test_clamp_sleep_never_overruns_the_budget
      deadline = Deadline.after(0, 100)

      assert_equal 30, deadline.clamp_sleep(0, 30)
      assert_equal 20, deadline.clamp_sleep(80, 30)
      assert_equal 0, deadline.clamp_sleep(200, 30)
      assert_equal 30, Deadline.never.clamp_sleep(0, 30)
    end

    def test_a_deadline_holds_nil_or_an_instant_in_the_duration_range
      assert_nil Deadline.never.expires_at
      assert_equal DURATION_MAX, Deadline.new(DURATION_MAX).expires_at
      assert_equal Deadline.new(nil), Deadline.never
      assert_equal Deadline.new(expires_at: 5), Deadline.at(5)
      assert_predicate Deadline.at(5), :frozen?
      assert_equal 'expires_at must be in 0..=Duration::MAX nanoseconds',
                   assert_raises(RangeError) { Deadline.new(-1) }.message
      assert_raises(RangeError) { Deadline.new(DURATION_MAX + 1) }
      assert_equal 'expires_at must be an Integer', assert_raises(TypeError) { Deadline.new(1.5) }.message
    end

    # Every argument is checked and named before the core runs.
    def test_arguments_are_integer_nanoseconds_named_for_the_core_parameter
      deadline = Deadline.at(100)
      {
        -> { Deadline.at(-1) } => 'instant',
        -> { Deadline.after(-1, 0) } => 'now',
        -> { Deadline.after(0, DURATION_MAX + 1) } => 'timeout',
        -> { deadline.remaining(-1) } => 'now',
        -> { deadline.derive(0, -1) } => 'timeout',
        -> { deadline.extend_by(-1) } => 'by',
        -> { deadline.clamp_sleep(0, -1) } => 'requested'
      }.each do |call, name|
        assert_equal "#{name} must be in 0..=Duration::MAX nanoseconds", assert_raises(RangeError, &call).message
      end
      [-> { deadline.check(0.5) }, -> { deadline.expired?('1') }, -> { Deadline.after(0, 1r) }].each do |call|
        assert_raises(TypeError, &call)
      end
    end

    def test_values_are_sealed
      assert_raises(NoMethodError) { Deadline.allocate }
      assert_raises(TypeError) { Class.new(Deadline) }
    end

    # The native side is the authority: only exactly a Deadline is read, its
    # expiry from the struct and checked again, whatever its readers say.
    def test_the_binding_reads_only_an_exact_deadline_and_rechecks_it
      deadline = Deadline.at(100)
      assert_raises(TypeError) { deadline.tighten(100) }
      assert_raises(TypeError) { deadline.tighten(Struct.new(:expires_at).new(5)) }

      assert_equal 100, with_lying_reader { Deadline.never.tighten(Deadline.at(100)).to_h[:expires_at] }

      # Marshal.load skips initialize: an expiry of -1 gets in, and is
      # refused the moment the binding reads it.
      forged = Marshal.load(Marshal.dump(Deadline.at(1)).sub("i\x06".b, "i\xFA".b))
      assert_equal(-1, forged.expires_at)
      assert_raises(RangeError) { forged.remaining(0) }
      assert_raises(RangeError) { Deadline.never.tighten(forged) }
    end

    # The class is pinned at init, so a compaction moves nothing it builds.
    def test_every_value_path_survives_gc_compact
      skip 'GC.compact is not available' unless GC.respond_to?(:compact)
      GC.compact

      assert_equal Deadline.at(5), Deadline.after(0, 5)
      assert_equal Deadline.at(1), Deadline.at(5).tighten(Deadline.at(1))
      assert_equal Deadline.never, Deadline.never.extend_by(1)
    end

    private

    # A redefined reader is ignored: the binding reads the struct.
    def with_lying_reader
      Deadline.alias_method(:__honest, :expires_at)
      Deadline.define_method(:expires_at) { 1 }
      yield
    ensure
      Deadline.alias_method(:expires_at, :__honest)
      Deadline.remove_method(:__honest)
    end
  end
end
