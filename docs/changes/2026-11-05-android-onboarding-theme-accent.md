# Android onboarding uses the theme accent (M9)

Date: 2026-11-05

## Change

The first-run setup screens (welcome hero and pulse rings, page badges, signed-in
check, completion check) drew a fixed teal "brand" colour (`#6ddbc1` / `#006f62`)
whatever theme was active. They now use the active theme's accent
(`MaterialTheme.colorScheme.primary`), adjusted only as far as needed for
4.5:1 contrast against the page background. The hard-coded brand tokens were
removed. A palette without an accent now falls back to its text colour instead of
a fixed teal (`#80cbc4`).

Shared palettes are unchanged: the built-in Vidcue Dark/Light palettes still
define a teal accent, so the setup screens stay teal under those themes.

## Checks

- `MediaDesignTest` updated: readable-brand contrast is checked for several theme
  accents across a grey ramp of surfaces.
- Not run here: the container has no Android SDK (the Gradle Android plugin
  could not be resolved offline). Compile and unit tests rely on CI; no
  emulator or device check was done.
