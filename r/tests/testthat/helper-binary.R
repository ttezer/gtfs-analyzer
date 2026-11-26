# Tests that run the CLI skip when no executable is found, so the package can be
# checked without one (CRAN, users). CI sets GTFSANALYZER_REQUIRE_BINARY=true:
# there a missing executable must fail the run instead of silently skipping.
skip_without_binary <- function() {
  if (nzchar(ga_binary(required = FALSE))) return(invisible(TRUE))
  if (identical(tolower(Sys.getenv("GTFSANALYZER_REQUIRE_BINARY")), "true")) {
    fail(paste0("GTFS Analyzer executable not found but GTFSANALYZER_REQUIRE_BINARY=true ",
                "(GTFS_ANALYZER_BIN=", Sys.getenv("GTFS_ANALYZER_BIN"), ")"))
    skip("executable required but missing (failure recorded above)")
  }
  skip("GTFS Analyzer executable not available")
}
