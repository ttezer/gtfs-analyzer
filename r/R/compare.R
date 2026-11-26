#' Compare two versions of a GTFS feed
#'
#' Validates both feeds (or takes existing [ga_analyze()] results) and reports
#' per rule how the number of findings changed, plus the change in scores.
#' Useful before and after editing a timetable.
#'
#' @param before,after Paths to GTFS ZIP archives, or `ga_result` objects.
#' @param ... Passed to [ga_analyze()] when a path is given.
#' @return An object of class `ga_comparison`: a list with `rules` (data frame
#'   with `rule_id`, `severity`, `before`, `after`, `change` and `state`, one of
#'   `"new"`, `"resolved"`, `"increased"`, `"decreased"`), `scores` (data frame
#'   of before/after/delta) and `publishable` (before/after).
#' @examples
#' if (nzchar(ga_binary(required = FALSE))) {
#'   f <- system.file("extdata", "sample-feed.zip", package = "gtfsanalyzer")
#'   ga_compare(f, f)
#' }
#' @export
ga_compare <- function(before, after, ...) {
  if (!inherits(before, "ga_result")) before <- ga_analyze(before, ...)
  if (!inherits(after, "ga_result")) after <- ga_analyze(after, ...)

  count <- function(res) {
    n <- res$notices
    if (nrow(n) == 0) return(data.frame(rule_id = character(), severity = character(), n = integer()))
    stats::aggregate(list(n = n$id), by = list(rule_id = n$rule_id, severity = n$severity), FUN = length)
  }
  m <- merge(count(before), count(after), by = c("rule_id", "severity"),
             all = TRUE, suffixes = c(".before", ".after"))
  names(m)[names(m) == "n.before"] <- "before"
  names(m)[names(m) == "n.after"] <- "after"
  m$before[is.na(m$before)] <- 0L
  m$after[is.na(m$after)] <- 0L
  m$change <- m$after - m$before
  m <- m[m$change != 0, , drop = FALSE]
  m$state <- ifelse(m$before == 0, "new",
             ifelse(m$after == 0, "resolved",
             ifelse(m$change > 0, "increased", "decreased")))
  m <- m[order(-abs(m$change), m$rule_id), , drop = FALSE]
  rownames(m) <- NULL

  scores <- data.frame(score = names(before$scores),
                       before = unname(before$scores),
                       after = unname(after$scores[names(before$scores)]))
  scores$delta <- scores$after - scores$before

  structure(
    list(rules = m, scores = scores,
         publishable = c(before = before$publishable, after = after$publishable)),
    class = "ga_comparison"
  )
}

#' @export
print.ga_comparison <- function(x, ...) {
  cat("<ga_comparison>\n")
  cat("  publishable: ", x$publishable[["before"]], " -> ", x$publishable[["after"]], "\n", sep = "")
  s <- x$scores[x$scores$score == "score", ]
  cat("  score:       ", format(s$before), " -> ", format(s$after), "\n", sep = "")
  st <- table(factor(x$rules$state, levels = c("new", "resolved", "increased", "decreased")))
  cat("  rules:       ", paste(names(st), st, sep = "=", collapse = " "), "\n", sep = "")
  invisible(x)
}
