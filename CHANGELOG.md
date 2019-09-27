# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.0 - 2019-09-27

###

- `EMatrix` now supports a Gaussian blur of escape values.

### Changed

- Color selection uses closer phase offsets, resulting in more muted browns and blues.

### Internals

- Key handling has been deduplicated across tui and termion backends.
- Screenshot logic has been coalesced and cleaned up.
- `RenderContext` has been renamed `Rctx`.
- Fractal related structs have been renamed, avoiding the misleading holomorphic term.
- The pixel coloring function has been pushed into `Rctx`.

## 0.3.0 - 2019-09-03

### Added

- Fractal exponents can be changed at runtime.

### Changed

- Julia and Mandelbrot Escape iterations are now smoothed.

## 0.2.0 - 2019-09-03

### Added

- Basic CLI options - `--tui tui` and `--tui termion`
- TUI based rendering

### Fixed

- Height/width inversion was inverted, which caused tui based rendering to generate garbage. This ordering has been fixed.

## 0.1.0 - 2019-08-18

This is the first version of the mandelbrot explorer. This release serves to
mark a known good state before the application is refactored to use tui.
