# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'rb_sys/extensiontask'

GEMSPEC = Gem::Specification.load('deadline_machines.gemspec')

SUPPORTED_NATIVE_PLATFORMS = %w[
  arm64-darwin
  x86_64-darwin
  aarch64-linux
  x86_64-linux
  x86_64-linux-musl
].freeze

native_ext = RbSys::ExtensionTask.new('deadline_machines_native', GEMSPEC) do |ext|
  ext.lib_dir = 'lib/deadline_machines_native'
  ext.tmp_dir = 'tmp/rb_sys'
  ext.cross_platform = SUPPORTED_NATIVE_PLATFORMS if ENV.key?('RUBY_TARGET')
end

# A single-version cross build stages its binary at the host binary's path,
# which the gem package depends on, so packaging would also compile for the
# host and fail where the host linker can't target the platform.
if ENV.key?('RUBY_TARGET')
  host_binary = "#{native_ext.lib_dir}/#{native_ext.name}.#{RbConfig::CONFIG['DLEXT']}"
  Rake::Task[host_binary].clear_prerequisites if Rake::Task.task_defined?(host_binary)
end

desc 'Compile and run the Ruby tests'
task test: :compile do
  sh 'padded_cell'
end

desc 'Test the Rust core under every feature it ships'
task :cargo do
  ['', 'alloc', 'tokio', 'tokio-util'].each do |features|
    sh 'cargo', 'test', '-p', 'deadline-machines', '--no-default-features', '--features', features
  end
end

desc 'Validate the RBS signatures'
task :rbs do
  sh 'rbs', '-I', 'sig', 'validate'
end

task default: %i[test cargo rbs]

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
