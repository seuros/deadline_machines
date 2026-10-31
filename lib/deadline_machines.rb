# frozen_string_literal: true

require_relative 'deadline_machines/version'
require_relative 'deadline_machines/errors'
require_relative 'deadline_machines/clock'
require_relative 'deadline_machines/deadline'

module DeadlineMachines
  FIBER_KEY = :deadline_machines_deadline

  class << self
    attr_writer :clock

    def clock
      @clock ||= MonotonicClock
    end

    def current
      Fiber[FIBER_KEY]
    end

    def with_deadline(seconds = nil, &block)
      ambient = current
      return ambient.derive(seconds) { |child| bind(child, &block) } if ambient && seconds
      return bind(Deadline.after(seconds, clock: clock), &block) if seconds

      bind(Deadline.never(clock: clock), &block)
    end

    def bind(deadline)
      previous = Fiber[FIBER_KEY]
      Fiber[FIBER_KEY] = deadline
      begin
        yield deadline
      ensure
        Fiber[FIBER_KEY] = previous
      end
    end

    def check!
      current&.check!
    end

    def remaining
      current&.remaining || Float::INFINITY
    end

    def clamp_sleep(seconds)
      current ? current.clamp_sleep(seconds) : seconds
    end

    def deadline?(object)
      object.respond_to?(:remaining) && object.respond_to?(:done?) && object.respond_to?(:check!)
    end
  end
end
