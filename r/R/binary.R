# Locating and installing the GTFS Analyzer executable.

# Engine version this package release is built and tested against.
ga_engine_version <- "0.22.0"

# SHA256 of each release archive, per engine version and platform. Filled from
# the release's SHA256SUMS file when the pinned engine version changes. A
# version or platform without an entry cannot be installed by `ga_install()`:
# there is deliberately no unverified download path.
ga_checksums <- list(
  # "0.23.0" = c(
  #   "x86_64-linux"   = "<sha256>",
  #   "aarch64-macos"  = "<sha256>",
  #   "x86_64-macos"   = "<sha256>",
  #   "x86_64-windows" = "<sha256>"
  # )
)

ga_release_url <- "https://github.com/ttezer/gtfs-analyzer/releases/download"

ga_platform <- function() {
  sys <- Sys.info()[["sysname"]]
  arch <- R.version$arch
  arch <- if (arch %in% c("aarch64", "arm64")) "aarch64" else
    if (arch %in% c("x86_64", "amd64")) "x86_64" else arch
  os <- switch(sys, Darwin = "macos", Linux = "linux", Windows = "windows", tolower(sys))
  paste(arch, os, sep = "-")
}

ga_exe_name <- function() {
  if (.Platform$OS.type == "windows") "gtfs-analyzer.exe" else "gtfs-analyzer"
}

ga_cache_dir <- function(version) {
  file.path(tools::R_user_dir("gtfsanalyzer", which = "cache"), version)
}

#' Locate the GTFS Analyzer executable
#'
#' Looks, in order, at the `GTFS_ANALYZER_BIN` environment variable, the
#' `gtfsanalyzer.bin` option, the copy installed by [ga_install()] for the
#' pinned engine version, and finally `gtfs-analyzer` on the `PATH`.
#'
#' @param required If `TRUE`, signal an error when no executable is found.
#' @return The path to the executable, or `""` when none is found and
#'   `required` is `FALSE`.
#' @examples
#' ga_binary(required = FALSE)
#' @export
ga_binary <- function(required = TRUE) {
  candidates <- c(
    Sys.getenv("GTFS_ANALYZER_BIN", ""),
    getOption("gtfsanalyzer.bin", ""),
    file.path(ga_cache_dir(ga_engine_version), ga_exe_name()),
    unname(Sys.which("gtfs-analyzer"))
  )
  for (path in candidates) {
    if (nzchar(path) && file.exists(path)) return(normalizePath(path))
  }
  if (required) {
    stop("GTFS Analyzer executable not found. Run `ga_install()`, or set the ",
         "GTFS_ANALYZER_BIN environment variable to an existing executable.",
         call. = FALSE)
  }
  ""
}

#' Download and install the GTFS Analyzer executable
#'
#' Downloads the release archive for the current platform from GitHub,
#' verifies its SHA256 checksum against the value pinned in this package and
#' unpacks it into the user cache directory ([tools::R_user_dir()]). Nothing
#' is downloaded unless this function is called. Verification needs the
#' 'openssl' package; without it the function stops before downloading.
#'
#' @param version Engine version to install. Only versions with a pinned
#'   checksum can be installed.
#' @param force Reinstall even if the executable is already present.
#' @return The path to the installed executable, invisibly.
#' @examples
#' \dontrun{
#' ga_install()
#' }
#' @export
ga_install <- function(version = ga_engine_version, force = FALSE) {
  dest <- ga_cache_dir(version)
  exe <- file.path(dest, ga_exe_name())
  if (file.exists(exe) && !force) return(invisible(exe))

  if (!requireNamespace("openssl", quietly = TRUE)) {
    stop("Checksum verification needs the 'openssl' package. ",
         "Install it with install.packages(\"openssl\") and try again.",
         call. = FALSE)
  }
  platform <- ga_platform()
  sums <- ga_checksums[[version]]
  expected <- if (is.null(sums)) NA_character_ else unname(sums[platform])
  if (is.na(expected)) {
    stop("No pinned checksum for engine ", version, " on ", platform, ". ",
         "Set GTFS_ANALYZER_BIN to an executable you built or downloaded yourself.",
         call. = FALSE)
  }

  asset <- paste0("gtfs-analyzer-", platform,
                  if (grepl("windows$", platform)) ".zip" else ".tar.gz")
  url <- paste(ga_release_url, paste0("v", version), asset, sep = "/")
  archive <- tempfile(fileext = if (grepl("windows$", platform)) ".zip" else ".tar.gz")
  on.exit(unlink(archive), add = TRUE)
  utils::download.file(url, archive, mode = "wb", quiet = TRUE)

  actual <- as.character(openssl::sha256(file(archive, open = "rb")))
  if (!identical(tolower(actual), tolower(expected))) {
    stop("Checksum mismatch for ", asset, ": expected ", expected, ", got ",
         actual, ". The file was not installed.", call. = FALSE)
  }

  dir.create(dest, recursive = TRUE, showWarnings = FALSE)
  if (grepl("windows$", platform)) {
    utils::unzip(archive, exdir = dest)
  } else {
    utils::untar(archive, exdir = dest)
    Sys.chmod(exe, mode = "0755")
  }
  if (!file.exists(exe)) stop("Archive did not contain ", ga_exe_name(), call. = FALSE)
  invisible(exe)
}
