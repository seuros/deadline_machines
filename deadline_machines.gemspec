# frozen_string_literal: true

require_relative 'lib/deadline_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'deadline_machines'
  spec.version = DeadlineMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']

  spec.summary = 'Deadline propagation and cooperative cancellation for Ruby'
  spec.description = 'A monotonic deadline token that derives child budgets, cancels down its tree, and raises ' \
                     'only at checkpoints you choose. The shared time contract for the machines family: retries, ' \
                     'circuit breakers and throttles read one budget instead of each keeping its own clock.'
  spec.homepage = 'https://github.com/seuros/deadline_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'

  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['changelog_uri'] = "#{spec.homepage}/blob/main/CHANGELOG.md"
  spec.metadata['bug_tracker_uri'] = "#{spec.homepage}/issues"
  spec.metadata['rubygems_mfa_required'] = 'true'

  spec.files = Dir.glob(%w[
                          lib/**/*.rb
                          sig/**/*.rbs
                          ext/**/*.{rs,toml}
                          Cargo.toml
                          README.md
                          CHANGELOG.md
                          LICENSE.txt
                        ]).select { |f| File.exist?(f) }
                   .reject { |f| f =~ %r{/tests(/|\.rs\z)} }
  spec.require_paths = ['lib']
end
