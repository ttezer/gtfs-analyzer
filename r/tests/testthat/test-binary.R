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

test_that("ga_verify_sha256 accepts a matching digest and rejects a wrong one", {
  skip_if_not_installed("openssl")
  path <- tempfile()
  writeBin(charToRaw("abc"), path)
  on.exit(unlink(path))
  abc <- "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  expect_true(gtfsanalyzer:::ga_verify_sha256(path, abc))
  expect_true(gtfsanalyzer:::ga_verify_sha256(path, toupper(abc)))
  expect_error(gtfsanalyzer:::ga_verify_sha256(path, strrep("0", 64)), "Checksum mismatch")
})

test_that("every pinned checksum is a 64-digit hex string for the four platforms", {
  for (version in names(gtfsanalyzer:::ga_checksums)) {
    sums <- gtfsanalyzer:::ga_checksums[[version]]
    expect_setequal(names(sums), c("x86_64-linux", "aarch64-macos", "x86_64-macos", "x86_64-windows"))
    expect_true(all(grepl("^[0-9a-f]{64}$", sums)))
  }
  expect_true(gtfsanalyzer:::ga_engine_version %in% names(gtfsanalyzer:::ga_checksums))
})
