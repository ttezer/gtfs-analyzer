#' List the validator's rules
#'
#' @param lang Language of titles: one of `"en"`, `"tr"`, `"ja"`, `"fr"`, `"es"`.
#' @return A data frame with one row per rule: `id`, `severity`, `class`,
#'   `authority_source`, `base_effort` and `title`.
#' @examples
#' if (nzchar(ga_binary(required = FALSE))) {
#'   head(ga_rules())
#' }
#' @export
ga_rules <- function(lang = "en") {
  lang <- match.arg(lang, c("en", "tr", "ja", "fr", "es"))
  run <- ga_run(c("rules", "--json", "--lang", lang))
  on.exit(unlink(run$out), add = TRUE)
  if (run$status != 0L || !file.exists(run$out)) {
    stop("GTFS Analyzer `rules` failed (exit code ", run$status, "). ", run$stderr, call. = FALSE)
  }
  rules <- jsonlite::fromJSON(run$out, simplifyVector = FALSE)
  col <- function(name) vapply(rules, function(r) as.character(ga_null(r[[name]], NA_character_)), character(1))
  data.frame(
    id = col("id"), severity = col("severity"), class = col("class"),
    authority_source = col("authority_source"),
    base_effort = vapply(rules, function(r) as.numeric(ga_null(r$base_effort)), numeric(1)),
    title = col("title"),
    stringsAsFactors = FALSE
  )
}
