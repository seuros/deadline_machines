# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'minitest/test_task'
require 'shellwords'

Minitest::TestTask.create(:minitest) do |t|
  t.libs << 'test'
  t.libs << 'lib'
  t.test_globs = ['test/**/*_test.rb']
end

if RUBY_ENGINE == 'ruby'
  desc 'Run the test suite'
  task :test do
    sh 'padded_cell', *Shellwords.split(ENV.fetch('TESTOPTS', ''))
  end
else
  task test: :minitest
end

desc 'Run the Rust core crate tests'
task :cargo do
  sh 'cargo', 'test', '--manifest-path', 'ext/deadline_machines_native/core/Cargo.toml'
end

task default: %i[test cargo]

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
