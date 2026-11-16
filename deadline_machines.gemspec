# frozen_string_literal: true

require_relative 'lib/deadline_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'deadline_machines'
  spec.version = DeadlineMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']

  spec.summary = 'Deadline propagation and cooperative cancellation for Ruby'
  spec.description = 'A monotonic deadline that derives child budgets, cancels down its tree, and raises ' \
                     'only at checkpoints you choose. The arithmetic comes from the no_std deadline-machines ' \
                     'crate; Ruby owns the cancellation tree and the ambient scope. The shared time contract ' \
                     'for the machines family: retries, circuit breakers and throttles read one budget.'
  spec.homepage = 'https://github.com/seuros/deadline_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'

  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['changelog_uri'] = "#{spec.homepage}/blob/main/CHANGELOG.md"
  spec.metadata['bug_tracker_uri'] = "#{spec.homepage}/issues"
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'deadline_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/deadline_machines_native/ffi/Cargo.toml'

  spec.files = Dir.glob(%w[
                          lib/**/*.rb
                          sig/**/*.rbs
                          ext/**/*.{rb,rs,toml,md}
                          Cargo.toml
                          Cargo.lock
                          README.md
                          CHANGELOG.md
                          LICENSE.txt
                        ]).select { |f| File.exist?(f) }
                   .reject { |f| f =~ %r{/tests(/|\.rs\z)} }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/deadline_machines_native/extconf.rb']

  # A source install runs extconf.rb, which requires rb_sys/mkmf, and RubyGems
  # clears GEM_PATH while building the extension. Without this the build only
  # works where rb_sys happens to be installed globally.
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
