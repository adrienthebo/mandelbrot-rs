# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 - 2019-09-03

### Added

- Basic CLI options - `--tui tui` and `--tui termion`
- TUI based rendering

### Fixed

- Height/width inversion was inverted, which caused tui based rendering to generate garbage. This ordering has been fixed.

## 0.1.0 - 2019-08-18

This is the first version of the mandelbrot explorer. This release serves to
mark a known good state before the application is refactored to use tui.
