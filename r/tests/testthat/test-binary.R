test_that("GTFS_ANALYZER_BIN takes precedence", {
  fake <- tempfile()
  file.create(fake)
  withr_env <- Sys.getenv("GTFS_ANALYZER_BIN", NA)
  on.exit(if (is.na(withr_env)) Sys.unsetenv("GTFS_ANALYZER_BIN") else Sys.setenv(GTFS_ANALYZER_BIN = withr_env))
  Sys.setenv(GTFS_ANALYZER_BIN = fake)
  expect_identical(ga_binary(), normalizePath(fake))
})

test_that("ga_install refuses versions without a pinned checksum", {
  skip_if_not_installed("openssl")
  expect_error(ga_install(version = "0.0.0", force = TRUE), "No pinned checksum")
})

test_that("platform string uses release asset naming", {
  expect_match(gtfsanalyzer:::ga_platform(), "^(x86_64|aarch64)-(linux|macos|windows)$")
})
