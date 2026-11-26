sample_feed <- function() system.file("extdata", "sample-feed.zip", package = "gtfsanalyzer")
skip_without_binary <- function() skip_if_not(nzchar(ga_binary(required = FALSE)), "GTFS Analyzer executable not available")

test_that("ga_analyze returns verdict, scores and a notice table", {
  skip_without_binary()
  res <- ga_analyze(sample_feed(), today = "20260601")
  expect_s3_class(res, "ga_result")
  expect_identical(res$status, "ok")
  expect_true(res$publishable)
  expect_named(res$scores, c("score", "pub_score", "spec_score", "interop_score",
                             "quality_score", "analytics_score"))
  expect_s3_class(res$notices, "data.frame")
  expect_true(all(c("rule_id", "severity", "message") %in% names(res$notices)))
  expect_gt(nrow(res$notices), 0)
  expect_s3_class(summary(res), "data.frame")
  expect_output(print(res), "publishable: TRUE")
})

test_that("an unreadable archive raises a gtfsanalyzer_fatal condition", {
  skip_without_binary()
  bad <- tempfile(fileext = ".zip")
  writeLines("not a zip", bad)
  expect_error(ga_analyze(bad), class = "gtfsanalyzer_fatal")
})

test_that("a missing feed path is rejected before running the validator", {
  expect_error(ga_analyze(tempfile(fileext = ".zip")), "existing GTFS ZIP")
})

test_that("comparing a feed with itself reports no rule changes", {
  skip_without_binary()
  cmp <- ga_compare(sample_feed(), sample_feed(), today = "20260601")
  expect_s3_class(cmp, "ga_comparison")
  expect_equal(nrow(cmp$rules), 0)
  expect_true(all(cmp$scores$delta == 0))
})

test_that("ga_rules lists the registry", {
  skip_without_binary()
  rules <- ga_rules()
  expect_gt(nrow(rules), 100)
  expect_false(anyDuplicated(rules$id) > 0)
})
