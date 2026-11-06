# Changelog

## [0.1.3](https://github.com/seuros/deadline_machines/compare/deadline_machines/v0.1.2...deadline_machines/v0.1.3) (2026-11-06)


### Features

* await cancellation and bridge to tokio-util tokens ([72118dc](https://github.com/seuros/deadline_machines/commit/72118dc316a4281f9fbcbb82c536e72b35cac0c2))


### Bug Fixes

* keep test modules out of the packaged crate ([2719800](https://github.com/seuros/deadline_machines/commit/27198003e6883be4eab975c15cd0a3e14614d808))

## [0.1.2](https://github.com/seuros/deadline_machines/compare/deadline_machines/v0.1.1...deadline_machines/v0.1.2) (2026-11-03)


### Features

* add optional Tokio deadline adapter ([6a36628](https://github.com/seuros/deadline_machines/commit/6a366288d4738f99affc172c62e6c49ca5caae37))
* pass a rubygems OTP through the environment ([d21d1b1](https://github.com/seuros/deadline_machines/commit/d21d1b13e22172eed51081e971342bd70f99f437))


### Bug Fixes

* a republished platform gem is skipped, not fatal ([7e12525](https://github.com/seuros/deadline_machines/commit/7e12525bfebb0d708b83784b2cb8a9e60404794b))
* collect the built gems whatever depth the download puts them at ([d5330ad](https://github.com/seuros/deadline_machines/commit/d5330ad2c7789ab810b73e1acb5ea2aa749c9a7c))
* the release tag carries the gem name, and the gate uses the aggregate output ([a4e0f34](https://github.com/seuros/deadline_machines/commit/a4e0f34ac239c046ed6fb1edb0ab6b6027fcf48c))

## [0.1.1](https://github.com/seuros/deadline_machines/compare/deadline_machines-v0.1.0...deadline_machines/v0.1.1) (2026-11-01)


### Features

* **core:** add Scope, RBS signatures, release config and CI ([24018c3](https://github.com/seuros/deadline_machines/commit/24018c39edcb12634f92057d699e1f2ad936ad4a))
* deadline propagation and cooperative cancellation ([9858496](https://github.com/seuros/deadline_machines/commit/9858496f48cc0eca1edf8df613f9974fa15abba2))


### Bug Fixes

* a block scope lets go of its child's retention too ([e5502c2](https://github.com/seuros/deadline_machines/commit/e5502c29ba21306ef386303e465195d27cb5d643))
* a cancellation racing a registration no longer strands the child ([e387b71](https://github.com/seuros/deadline_machines/commit/e387b71e6464bb5a6b35ec17b68e898231a8eca7))
* a failed gem push fails the release ([26678da](https://github.com/seuros/deadline_machines/commit/26678da9039c72be8457e53f3cc4473b6394f5c1))
* a nested scope without a limit inherits the ambient budget ([f88a0d5](https://github.com/seuros/deadline_machines/commit/f88a0d56497ee199664bd96fa479d6262abcc245))
* a parent no longer owns every deadline derived from it ([41d609c](https://github.com/seuros/deadline_machines/commit/41d609c623a480b94a3f2789c0d7c3dbf2920ee9))
* an observer deep in a chain is held all the way to the root ([9abeb8d](https://github.com/seuros/deadline_machines/commit/9abeb8d311f85991d4eed2d4f932b147e59ed574))
* detachment stops the retention walk, not just its first step ([44ff4d1](https://github.com/seuros/deadline_machines/commit/44ff4d108a6dc139a7731e50432d5010b2f7c2cd))
* one failing cancel observer no longer silences its peers ([a9ddd70](https://github.com/seuros/deadline_machines/commit/a9ddd70d0d1355aacfc0170d9b874bd9c90e3309))
* one lock per tree, so a retention chain walk is atomic ([eec49b9](https://github.com/seuros/deadline_machines/commit/eec49b9ff6a79930cfbb4cbb8de398ad7b75efaa))
* ship the core's test files so the packaged crate builds ([0340bc6](https://github.com/seuros/deadline_machines/commit/0340bc6ec958cb1983212f4cd0b9943cc36aef5c))

## 0.1.0

- Initial release.
- `DeadlineMachines::Deadline`: monotonic budgets, `derive` for child scopes that
  cannot outlive their parent, cancellation that flows down a tree, and `check!`
  as the only place an error is raised.
- `DeadlineMachines.with_deadline` for a fiber-scoped ambient deadline.
- `DeadlineMachines::TestClock` for deterministic tests across the family.
- `deadline-machines` core crate: the same arithmetic and cancellation tree in
  `no_std` Rust with zero dependencies, plus `Scope` pairing a budget with a
  cancellation handle. Builds and lints clean on bare metal.
- RBS signatures for the whole public API.
