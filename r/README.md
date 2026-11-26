# gtfsanalyzer

R interface to [GTFS Analyzer](https://github.com/ttezer/gtfs-analyzer), a fast
GTFS Schedule validator and analysis engine. No Java, Python or Rust toolchain
is needed: the package runs the prebuilt `gtfs-analyzer` executable.

```r
# install.packages("remotes")
remotes::install_github("ttezer/gtfs-analyzer", subdir = "r")

library(gtfsanalyzer)
ga_install()                 # checksum-verified download into the user cache
res <- ga_analyze("feed.zip")
res                          # verdict, score, findings by severity
summary(res)                 # findings per rule
head(res$notices)

ga_compare("before.zip", "after.zip")   # what changed after editing a timetable
```

To use an executable you built or downloaded yourself, set
`GTFS_ANALYZER_BIN=/path/to/gtfs-analyzer`.
