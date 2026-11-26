## Submission

This is a new submission.

## R CMD check results

0 errors | 0 warnings | 1 note

* checking CRAN incoming feasibility ... NOTE
  New submission

## Notes for the reviewer

* The package is an interface to the 'GTFS Analyzer' command line validator,
  an open-source executable (MIT) declared in SystemRequirements. Nothing is
  downloaded unless the user calls ga_install(). That function downloads the
  executable for the pinned engine version from the project's GitHub release,
  verifies its SHA-256 against a value pinned in the package (it stops if the
  suggested package 'openssl' is not available, so there is no unverified
  path) and writes it below tools::R_user_dir("gtfsanalyzer", "cache").
* The ga_install() example is wrapped in \dontrun{} because it downloads a
  file and writes to the user's cache directory.
* Tests and examples that need the executable are skipped when it is not
  available, so the package checks without network access or the executable.

## Test environments

* local macOS (aarch64), R 4.6.2
* GitHub Actions: ubuntu-latest, macos-latest, windows-latest (release R),
  with the executable built from source
