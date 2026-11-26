# Running the validator and turning its JSON output into R objects.

# Highest `schema_version` of the CLI JSON envelope this package understands.
# Output without the field comes from engines released before it was added.
ga_supported_schema <- 1L

ga_run <- function(args) {
  bin <- ga_binary()
  out <- tempfile(fileext = ".json")
  err <- tempfile(fileext = ".txt")
  on.exit(unlink(err), add = TRUE)
  status <- suppressWarnings(
    system2(bin, c(args, "-o", shQuote(out)), stdout = FALSE, stderr = err)
  )
  list(status = status, out = out, stderr = paste(readLines(err, warn = FALSE), collapse = "\n"))
}

ga_null <- function(x, default = NA) if (is.null(x)) default else x

ga_check_schema <- function(json) {
  v <- json$schema_version
  if (!is.null(v) && v > ga_supported_schema) {
    warning("GTFS Analyzer output uses schema_version ", v, "; this package ",
            "understands up to ", ga_supported_schema, ". Update 'gtfsanalyzer'.",
            call. = FALSE)
  }
}

ga_notices_df <- function(notices) {
  col <- function(name) {
    vapply(notices, function(n) {
      v <- n[[name]]
      if (is.null(v)) NA_character_ else as.character(v)
    }, character(1))
  }
  line <- vapply(notices, function(n) {
    v <- n$line
    if (is.null(v)) NA_integer_ else as.integer(v)
  }, integer(1))
  data.frame(
    id = col("id"), rule_id = col("rule_id"), severity = col("severity"),
    rule_class = col("rule_class"), entity_type = col("entity_type"),
    entity_id = col("entity_id"), file = col("file"), line = line,
    field = col("field"), title = col("title"), message = col("message"),
    remediation = col("remediation"),
    stringsAsFactors = FALSE
  )
}

#' Validate a GTFS feed
#'
#' Runs GTFS Analyzer on a GTFS Schedule ZIP archive and returns the
#' publishability verdict, quality scores and findings.
#'
#' The validator's exit code 1 only means that findings exist; it is not
#' treated as an error. A feed the validator cannot read at all (exit code 2)
#' signals an error of class `gtfsanalyzer_fatal`.
#'
#' @param feed Path to a GTFS ZIP archive.
#' @param lang Language of titles, messages and remediations: one of `"en"`,
#'   `"tr"`, `"ja"`, `"fr"` or `"es"`.
#' @param today Validation date as `"YYYYMMDD"`, or `NULL` for the current date.
#' @param max_notices_per_rule Keep at most this many findings per rule, or
#'   `NULL` for all. Scores and the verdict always cover every finding.
#' @return An object of class `ga_result`: a list with `publishable`,
#'   `scores` (named numeric), `notices` (data frame), `status`, `engine` and
#'   `raw` (the parsed JSON).
#' @examples
#' if (nzchar(ga_binary(required = FALSE))) {
#'   res <- ga_analyze(system.file("extdata", "sample-feed.zip", package = "gtfsanalyzer"))
#'   res
#' }
#' @export
ga_analyze <- function(feed, lang = "en", today = NULL, max_notices_per_rule = NULL) {
  if (!is.character(feed) || length(feed) != 1L || !file.exists(feed)) {
    stop("`feed` must be the path to an existing GTFS ZIP file.", call. = FALSE)
  }
  lang <- match.arg(lang, c("en", "tr", "ja", "fr", "es"))
  args <- c("validate", shQuote(normalizePath(feed)), "--json", "--lang", lang)
  if (!is.null(today)) args <- c(args, "--today", today)
  if (!is.null(max_notices_per_rule)) {
    args <- c(args, "--max-notices-per-rule", as.integer(max_notices_per_rule))
  }

  run <- ga_run(args)
  on.exit(unlink(run$out), add = TRUE)
  if (!file.exists(run$out) || file.size(run$out) == 0) {
    stop("GTFS Analyzer produced no output (exit code ", run$status, "). ",
         run$stderr, call. = FALSE)
  }
  json <- jsonlite::fromJSON(run$out, simplifyVector = FALSE)
  ga_check_schema(json)

  if (identical(json$status, "fatal")) {
    cond <- structure(
      class = c("gtfsanalyzer_fatal", "error", "condition"),
      list(message = paste0("GTFS Analyzer could not validate the feed (",
                            json$code, "): ", json$message),
           call = NULL, code = json$code, params = json$params)
    )
    stop(cond)
  }
  if (!run$status %in% c(0L, 1L)) {
    stop("GTFS Analyzer exited with code ", run$status, ". ", run$stderr, call. = FALSE)
  }

  r5 <- json$reports$r5
  scores <- vapply(c("score", "pub_score", "spec_score", "interop_score",
                     "quality_score", "analytics_score"),
                   function(k) as.numeric(ga_null(r5[[k]])), numeric(1))
  structure(
    list(
      feed = feed,
      status = json$status,
      publishable = isTRUE(json$reports$r1$publishable),
      scores = scores,
      notices = ga_notices_df(json$notices),
      engine = list(version = ga_null(json$engine_version),
                    commit = ga_null(json$engine_commit),
                    schema_version = ga_null(json$schema_version)),
      raw = json
    ),
    class = "ga_result"
  )
}

#' @export
print.ga_result <- function(x, ...) {
  cat("<ga_result> ", basename(x$feed), "\n", sep = "")
  cat("  status:      ", x$status, "\n", sep = "")
  cat("  publishable: ", x$publishable, "\n", sep = "")
  cat("  score:       ", format(x$scores[["score"]]), "\n", sep = "")
  cat("  findings:    ", nrow(x$notices), "\n", sep = "")
  if (nrow(x$notices) > 0) {
    sev <- table(factor(x$notices$severity,
                        levels = c("CRITICAL", "HIGH", "MEDIUM", "LOW", "INFO")))
    cat("  by severity: ", paste(names(sev), sev, sep = "=", collapse = " "), "\n", sep = "")
  }
  invisible(x)
}

#' @export
summary.ga_result <- function(object, ...) {
  n <- object$notices
  if (nrow(n) == 0) {
    return(data.frame(rule_id = character(), severity = character(),
                      rule_class = character(), count = integer()))
  }
  agg <- stats::aggregate(list(count = n$id),
                          by = list(rule_id = n$rule_id, severity = n$severity,
                                    rule_class = n$rule_class),
                          FUN = length)
  agg[order(-agg$count, agg$rule_id), , drop = FALSE]
}
